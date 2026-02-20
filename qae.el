;;; qae.el --- fast, friendly searching with ripgrep (qae fork of deadgrep)  -*- lexical-binding: t; -*-

;; Based on deadgrep.el by Wilfred Hughes
;; Copyright (C) 2018-2024  Wilfred Hughes

;; Original author: Wilfred Hughes <me@wilfred.me.uk>
;; URL: https://github.com/Wilfred/deadgrep
;; Keywords: tools
;; Version: 0.14
;; Package-Requires: ((emacs "25.1") (dash "2.12.0") (s "1.11.0") (spinner "1.7.3"))

;; This program is free software; you can redistribute it and/or modify
;; it under the terms of the GNU General Public License as published by
;; the Free Software Foundation, either version 3 of the License, or
;; (at your option) any later version.

;; This program is distributed in the hope that it will be useful,
;; but WITHOUT ANY WARRANTY; without even the implied warranty of
;; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
;; GNU General Public License for more details.

;; You should have received a copy of the GNU General Public License
;; along with this program.  If not, see <https://www.gnu.org/licenses/>.

;;; Commentary:

;; Perform text searches with the speed of ripgrep and the comfort of
;; Emacs.  This is a bespoke mode that does not rely on
;; compilation-mode, but tries to be a perfect fit for ripgrep.

;; Install from MELPA, then `M-x qae' will do a search!

;;; Code:

(require 'cl-lib)
(require 's)
(require 'dash)
(require 'spinner)
(require 'project)

(defgroup qae nil
  "A powerful text search UI using ripgrep."
  :group 'tools
  :group 'matching)

(defcustom qae-executable
  "qae"
  "The qae executable used by qae.el.
This will be looked up on `exec-path' if it isn't an absolute
path to the binary."
  :type 'string
  :group 'qae)

(defcustom qae-max-buffers
  4
  "The maximum number of qae results buffers.

If the number of results buffers exceeds this value, qae
will kill results buffers. The least recently used buffers are
killed first.

To disable cleanup entirely, set this variable to nil."
  :type '(choice
          (number :tag "Maximum of buffers allowed")
          (const :tag "Disable cleanup" nil))
  :group 'qae)

(defcustom qae-project-root-function
  #'qae--project-root
  "Function called by `qae' to work out the root directory
to search from.

See also `qae-project-root-overrides'."
  :type 'function
  :group 'qae)

(defvar qae-project-root-overrides nil
  "An alist associating project directories with the desired
search directory.

This is useful for large repos where you only want to search a
subdirectory. It's also handy for nested repos where you want to
search from the parent.

This affects the behaviour of `qae--project-root', so this
variable has no effect if you change
`qae-project-root-function'.")

(defvar qae-history
  nil
  "A list of the previous search terms.")

(defvar qae-max-line-length
  500
  "Truncate lines if they are longer than this.

Emacs performance can be really poor with long lines, so this
ensures that searching minified files does not slow down movement
in results buffers.

In extreme cases (100KiB+ single-line files), we can get a stack
overflow on our regexp matchers if we don't apply this.")

(defcustom qae-display-buffer-function
  'switch-to-buffer-other-window
  "Function used to show the qae result buffer.

This function is called with one argument, the results buffer to
display."
  :type 'function
  :group 'qae)

(defface qae-meta-face
  '((t :inherit font-lock-comment-face))
  "Face used for qae UI text."
  :group 'qae)

(defface qae-filename-face
  '((t :inherit bold))
  "Face used for filename headings in results buffers."
  :group 'qae)

(defface qae-search-term-face
  '((t :inherit font-lock-variable-name-face))
  "Face used for the search term in results buffers."
  :group 'qae)

(defface qae-regexp-metachar-face
  '((t :inherit
       ;; TODO: I've seen a more appropriate face in some themes,
       ;; find out what to use instead here.
       font-lock-constant-face))
  "Face used for regexp metacharacters in search terms."
  :group 'qae)

(defface qae-match-face
  '((t :inherit match))
  "Face used for the portion of a line that matches the search term."
  :group 'qae)

(defvar-local qae--search-term nil)
;; Ensure this variable is ignored by `kill-all-local-variables' when
;; switching between `qae-mode' and `qae-edit-mode'.
(put 'qae--search-term 'permanent-local t)

(defvar-local qae--search-type 'string)
(put 'qae--search-type 'permanent-local t)
(defvar-local qae--search-case 'smart)
(put 'qae--search-case 'permanent-local t)
(defvar-local qae--file-type 'all)
(put 'qae--file-type 'permanent-local t)

(defvar-local qae--skip-if-hidden nil
  "Whether qae should ignore hidden files (e.g. files called .foo).")
(put 'qae--skip-if-hidden 'permanent-local t)
(defvar-local qae--skip-if-vcs-ignore 't
  "Whether qae should ignore files if they're listed in .gitignore.")
(put 'qae--skip-if-vcs-ignore 'permanent-local t)

(defvar-local qae--context nil
  "When set, also show context of results.
This is stored as a cons cell of integers (lines-before . lines-after).")
(put 'qae--context 'permanent-local t)
(defvar-local qae--initial-filename nil
  "The filename of the buffer that qae was started from.
Used to offer better default values for file options.")
(put 'qae--initial-filename 'permanent-local t)

(defvar-local qae--current-file nil
  "The file we're currently inserting results for.")
(defvar-local qae--spinner nil)
(defvar-local qae--remaining-output nil
  "We can't guarantee that our process filter will always receive whole lines.
We save the last line here, in case we need to append more text to it.")
(defvar-local qae--postpone-start nil
  "If non-nil, don't (re)start searches.")
(defvar-local qae--running nil
  "If non-nil, a search is still running.")
(defvar-local qae--result-count nil
  "The number of matches found for the current search.")

(defvar-local qae--debug-command nil)
(put 'qae--debug-command 'permanent-local t)
(defvar-local qae--debug-first-output nil)
(put 'qae--debug-first-output 'permanent-local t)

(defvar-local qae--imenu-alist nil
  "Alist that stores filename and position for each matched files.
It is used to create `imenu' index.")

(defconst qae--position-column-width 5)

(defconst qae--color-code
  (rx "\x1b[" (+ digit) "m")
  "Regular expression for an ANSI color code.")

(defvar qae--incremental-active nil)

(defun qae--insert-output (output &optional finished)
  "Propertize OUTPUT from rigrep and write to the current buffer."
  ;; If we had an unfinished line from our last call, include that.
  (when qae--remaining-output
    (setq output (concat qae--remaining-output output))
    (setq qae--remaining-output nil))

  (let ((inhibit-read-only t)
        (lines (s-lines output))
        prev-line-num)
    ;; Process filters run asynchronously, and don't guarantee that
    ;; OUTPUT ends with a complete line. Save the last line for
    ;; later processing.
    (unless finished
      (setq qae--remaining-output (-last-item lines))
      (setq lines (butlast lines)))

    (save-excursion
      (goto-char (point-max))
      (dolist (line lines)
        (cond
         ;; Ignore blank lines.
         ((s-blank? line))
         ;; Lines of just -- are used as a context separator when
         ;; calling ripgrep with context flags.
         ((string= line "--")
          (let ((separator "--"))
            ;; Try to make the separator length match the previous
            ;; line numbers.
            (when prev-line-num
              (setq separator
                    (s-repeat (log prev-line-num 10) "-")))
            (insert
             (propertize (concat separator "\n")
                         'face 'qae-meta-face
                         'qae-separator t))))
         ;; If we have a warning or don't have a color code, ripgrep
         ;; must be complaining about something (e.g. zero matches for
         ;; a glob, or permission denied on some directories).
         ((or
           (s-starts-with-p "WARNING: " line)
           (not (s-matches-p qae--color-code line)))
          (when qae--current-file
            (setq qae--current-file nil)
            (insert "\n"))
          (insert line "\n\n"))
         (t
          (-let* ((truncate-p (> (length line) qae-max-line-length))
                  (line
                   (if truncate-p
                       (substring line 0 qae-max-line-length)
                     line))
                  ((filename line-num content) (qae--split-line line))
                  (formatted-line-num
                   (s-pad-right qae--position-column-width " "
                                (number-to-string line-num)))
                  (pretty-line-num
                   (propertize formatted-line-num
                               'face 'qae-meta-face
                               'qae-filename filename
                               'qae-line-number line-num
                               'read-only t
                               'front-sticky t
                               'rear-nonsticky t))
                  (pretty-filename
                   (propertize filename
                               'face 'qae-filename-face
                               'qae-filename filename
                               'read-only t
                               'front-sticky t)))
            (cond
             ;; This is the first file we've seen, print the heading.
             ((null qae--current-file)
              (push (cons filename (point)) qae--imenu-alist)
              (insert pretty-filename "\n"))
             ;; This is a new file, print the heading with a spacer.
             ((not (equal qae--current-file filename))
              (push (cons filename (1+ (point))) qae--imenu-alist)
              (insert "\n" pretty-filename "\n")))
            (setq qae--current-file filename)

            ;; TODO: apply the invisible property if the user decided
            ;; to hide this filename before we finished finding
            ;; results in it.
            (insert pretty-line-num content)

            (when (null qae--result-count)
              (setq qae--result-count 0))
            (cl-incf qae--result-count)

            (when truncate-p
              (insert
               (propertize " ... (truncated)"
                           'face 'qae-meta-face)))
            (insert "\n")

            (setq prev-line-num line-num))))))))

(defcustom qae-finished-hook nil
  "Hook run when `qae' search is finished."
  :type 'hook
  :group 'qae)

(defun qae--process-sentinel (process output)
  "Update the qae buffer associated with PROCESS as complete."
  (let ((buffer (process-buffer process))
        (finished-p (string= output "finished\n")))
    (when (buffer-live-p buffer)
      (with-current-buffer buffer
        (setq qae--running nil)
        ;; rg has terminated, so stop the spinner.
        (spinner-stop qae--spinner)

        (qae--insert-output "" finished-p)

        ;; Report any errors that occurred.
        (unless (member output
                        (list
                         "exited abnormally with code 1\n"
                         "interrupt\n"
                         "finished\n"))
          (save-excursion
            (let ((inhibit-read-only t))
              (goto-char (point-max))
              (insert output))))

        (run-hooks 'qae-finished-hook)
        (unless qae--incremental-active
          (message "Qae finished"))))))

(defun qae--process-filter (process output)
  ;; Searches may see a lot of output, but it's really useful to have
  ;; a snippet of output when debugging. Store the first output received.
  (unless qae--debug-first-output
    (setq qae--debug-first-output output))

  ;; If we had an unfinished line from our last call, include that.
  (when qae--remaining-output
    (setq output (concat qae--remaining-output output))
    (setq qae--remaining-output nil))

  (when (buffer-live-p (process-buffer process))
    (with-current-buffer (process-buffer process)
      (qae--insert-output output))))

(defun qae--extract-regexp (pattern s)
  "Search for PATTERN in S, and return the content of the first group."
  (string-match pattern s)
  (match-string 1 s))

(defconst qae--filename-regexp
  (rx bos "\x1b[0m\x1b[3" (or "5" "6") "m"
      (? "./")
      (group (+? anything))
      "\x1b[")
  "Extracts the filename from a ripgrep line with ANSI color sequences.
We use the color sequences to extract the filename exactly, even
if the path contains colons.")

(defconst qae--line-num-regexp
  (rx "\x1b[32m" (group (+ digit)))
  "Extracts the line number from a ripgrep line with ANSI color sequences.
Ripgrep uses a unique color for line numbers, so we use that to
extract the linue number exactly.")

(defconst qae--line-contents-regexp
  (rx "\x1b[32m" (+ digit) "\x1b[0m" (or ":" "-") (group (* anything)))
  "Extract the line contents from a ripgrep line with ANSI color sequences.
Use the unique color for line numbers to ensure we start at the
correct colon.

Note that the text in the group will still contain color codes
highlighting which parts matched the user's search term.")

(defconst qae--hit-regexp
  (rx-to-string
   `(seq
     ;; A reset color code.
     "\x1b[0m"
     ;; Two color codes, bold and color (any order).
     (regexp ,qae--color-code)
     (regexp ,qae--color-code)
     ;; The actual text.
     (group (+? anything))
     ;; A reset color code again.
     "\x1b[0m"))
  "Extract the portion of a line found by ripgrep that matches the user's input.
This may occur multiple times in one line.")

(defun qae--split-line (line)
  "Split out the components of a raw LINE of output from rg.
Return the filename, line number, and the line content with ANSI
color codes replaced with string properties."
  (list
   (qae--extract-regexp qae--filename-regexp line)
   (string-to-number
    (qae--extract-regexp qae--line-num-regexp line))
   (qae--propertize-hits
    (qae--extract-regexp qae--line-contents-regexp line))))

(defun qae--escape-backslash (s)
  "Escape occurrences of backslashes in S.

This differs from `regexp-quote', which outputs a regexp pattern.
Instead, we provide a string suitable for REP in
`replace-regexp-in-string'."
  (s-replace "\\" "\\\\" s))

(defun qae--propertize-hits (line-contents)
  "Given LINE-CONTENTS from ripgrep, replace ANSI color codes
with a text face property `qae-match-face'."
  (replace-regexp-in-string
   qae--hit-regexp
   (lambda (s)
     (propertize
      (qae--escape-backslash (match-string 1 s))
      'face 'qae-match-face))
   line-contents))

(define-button-type 'qae-search-term
  'action #'qae--search-term
  'help-echo "Change search term")

(defun qae--search-prompt (&optional default)
  "The prompt shown to the user when starting a qae search."
  (let ((kind (if (eq qae--search-type 'regexp)
                  "by regexp" "for text")))
    (if default
        (format "Search %s (default %s): " kind default)
      (format "Search %s: " kind))))

(defun qae--search-term (_button)
  (qae-search-term))

(defun qae-search-term ()
  "Change the current search term and restart the search."
  (interactive)
  (setq qae--search-term
        (read-from-minibuffer
         (qae--search-prompt)
         qae--search-term))
  (rename-buffer
   (qae--buffer-name qae--search-term default-directory) t)
  (qae-restart))

(define-button-type 'qae-type
  'action #'qae--search-type
  'search-type nil
  'help-echo "Change search type")

(defun qae--search-type (button)
  (setq qae--search-type (button-get button 'search-type))
  (qae-restart))

(define-button-type 'qae-case
  'action #'qae--case
  'case nil
  'help-echo "Change case sensitivity")

(defun qae--case (button)
  (setq qae--search-case (button-get button 'case))
  (qae-restart))

(define-button-type 'qae-context
  'action #'qae--context
  'context nil
  'help-echo "Show/hide context around match")

(defun qae--context (button)
  ;; qae--context takes the value of (before . after) when set.
  (setq qae--context
        (cl-case (button-get button 'context)
          ((nil)
           nil)
          (before
           (cons
            (read-number "Show N lines before: ")
            (or (cdr-safe qae--context) 0)))
          (after
           (cons
            (or (car-safe qae--context) 0)
            (read-number "Show N lines after: ")))
          (t
           (error "Unknown context type"))))

  (qae-restart))

(defun qae--type-list ()
  "Query the rg executable for available file types."
  (let* ((output (with-output-to-string
                   (with-current-buffer standard-output
                     (process-file-shell-command
                      (format "%s --type-list" qae-executable)
                      nil '(t nil)))))
         (lines (s-lines (s-trim output)))
         (types-and-globs
          (--map
           (s-split (rx ": ") it)
           lines)))
    (-map
     (-lambda ((type globs))
       (list type (s-split (rx ", ") globs)))
     types-and-globs)))

(define-button-type 'qae-file-type
  'action #'qae--file-type
  'case nil
  'help-echo "Change file type")

(define-button-type 'qae-skip-hidden-type
  'action #'qae--skip-if-hidden
  'case nil
  'help-echo "Toggle whether to skip dotfiles")

(defun qae--skip-if-hidden (_button)
  (setq qae--skip-if-hidden (not qae--skip-if-hidden))
  (qae-restart))

(define-button-type 'qae-vcs-skip-type
  'action #'qae--skip-if-vcs-ignore
  'case nil
  'help-echo "Toggle whether to skip files listed in .gitignore")

(defun qae--skip-if-vcs-ignore (_button)
  (setq qae--skip-if-vcs-ignore (not qae--skip-if-vcs-ignore))
  (qae-restart))

(defun qae--format-file-type (file-type extensions)
  (let* ((max-exts 4)
         (truncated (> (length extensions) max-exts)))
    (when truncated
      (setq extensions
            (append (-take max-exts extensions)
                    (list "..."))))
    (format "%s (%s)"
            file-type
            (s-join ", " extensions))))

(defun qae--glob-regexp (glob)
  "Convert GLOB pattern to the equivalent elisp regexp."
  (let* ((i 0)
         (result "^"))
    (while (< i (length glob))
      (let* ((char (elt glob i)))
        (cond
         ;; . matches a literal . in globs.
         ((eq char ?.)
          (setq result (concat result "\\."))
          (cl-incf i))
         ;; ? matches a single char in globs.
         ((eq char ??)
          (setq result (concat result "."))
          (cl-incf i))
         ;; * matches zero or more of anything.
         ((eq char ?*)
          (setq result (concat result ".*"))
          (cl-incf i))
         ;; [ab] matches a literal a or b.
         ;; [a-z] matches characters between a and z inclusive.
         ;; [?] matches a literal ?.
         ((eq char ?\[)
          ;; Find the matching ].
          (let ((j (1+ i)))
            (while (and (< j (length glob))
                        (not (eq (elt glob j) ?\])))
              (cl-incf j))
            (cl-incf j)
            (setq result (concat result
                                 (substring glob i j)))
            (setq i j)))
         (t
          (setq result (concat result (char-to-string char)))
          (cl-incf i)))))
    (concat result "$")))

(defun qae--matches-globs-p (filename globs)
  "Return non-nil if FILENAME matches any glob pattern in GLOBS."
  (when filename
    (--any (string-match-p (qae--glob-regexp it) filename)
           globs)))

(defun qae--relevant-file-type (filename types-and-globs)
  "Try to find the most relevant item in TYPES-AND-GLOBS for FILENAME."
  (let (;; Find all the items in TYPES-AND-GLOBS whose glob match
        ;; FILENAME.
        (matching (-filter (-lambda ((_type globs))
                             (qae--matches-globs-p filename globs))
                           types-and-globs)))
    (->> matching
         ;; Prefer longer names, so "markdown" over "md" for the type
         ;; name.
         (-sort (-lambda ((type1 _) (type2 _))
                  (< (length type1) (length type2))))
         ;; Prefer types with more extensions, as they tend to be more
         ;; common languages (e.g. 'ocaml' over 'ml').
         (-sort (-lambda ((_ globs1) (_ globs2))
                  (< (length globs1) (length globs2))))
         ;; But prefer elisp over lisp for .el files.
         (-sort (-lambda ((type1 _) (type2 _))
                  ;; Return t if we're comparing elisp with lisp, nil
                  ;; otherwise.
                  (and (equal type1 "lisp")
                       (equal type2 "elisp"))))
         ;; Take the highest scoring matching.
         (-last-item))))

(defun qae--read-file-type (filename)
  "Read a ripgrep file type, defaulting to the type that matches FILENAME."
  (let* (;; Get the list of types we can offer.
         (types-and-globs (qae--type-list))
         ;; Build a list mapping the formatted types to the type name.
         (type-choices
          (-map
           (-lambda ((type globs))
             (list
              (qae--format-file-type type globs)
              type))
           types-and-globs))
         ;; Work out the default type name based on the filename.
         (default-type-and-globs
           (qae--relevant-file-type filename types-and-globs))
         (default
           (-when-let ((default-type default-globs) default-type-and-globs)
             (qae--format-file-type default-type default-globs)))
         ;; Prompt the user for a file type.
         (chosen
          (completing-read
           "File type: " type-choices nil t nil nil default)))
    (nth 1 (assoc chosen type-choices))))

(defun qae--read-file-glob ()
  (let*
      ((initial-value
        (cond
         ;; If we already have a glob pattern, edit it.
         ((eq (car-safe qae--file-type) 'glob)
          (cdr qae--file-type))
         ;; If the initial file had a file name of the form
         ;; foo.bar, offer *.bar as the initial glob.
         ((and qae--initial-filename
               (file-name-extension qae--initial-filename))
          (format "*.%s"
                  (file-name-extension qae--initial-filename)))
         (t
          "*")))
       (prompt
        (if (string= initial-value "*")
            ;; Show an example to avoid confusion with regexp syntax.
            "Glob (e.g. *.js): "
          "Glob: "))
       (glob
        (read-from-minibuffer
         prompt
         initial-value)))
    glob))

(defun qae--file-type (button)
  (let ((button-type (button-get button 'file-type)))
    (cond
     ((eq button-type 'all)
      (setq qae--file-type 'all))
     ((eq button-type 'type)
      (setq qae--file-type
            (cons 'type (qae--read-file-type qae--initial-filename))))
     ((eq button-type 'glob)
      (setq qae--file-type (cons 'glob (qae--read-file-glob))))
     (t
      (error "Unknown button type: %S" button-type))))
  (qae-restart))

(define-button-type 'qae-directory
  'action #'qae--directory
  'help-echo "Change base directory")

(defun qae--directory (_button)
  (qae-directory))

(defun qae-directory ()
  "Prompt the user for a new search directory, then restart the search."
  (interactive)
  (setq default-directory
        (expand-file-name
         (read-directory-name "Search files in: ")))
  (rename-buffer
   (qae--buffer-name qae--search-term default-directory)
   t)
  (qae-restart))

(defun qae-parent-directory ()
  "Restart the search in the parent directory."
  (interactive)
  (setq default-directory
        (file-name-directory (directory-file-name default-directory)))
  (rename-buffer
   (qae--buffer-name qae--search-term default-directory)
   t)
  (qae-restart))

(defun qae--button (text type &rest properties)
  ;; `make-text-button' mutates the string to add properties, so copy
  ;; TEXT first.
  (setq text (substring-no-properties text))
  (apply #'make-text-button text nil :type type properties))

(defcustom qae-extra-arguments
  '("--no-config")
  "List defining extra arguments passed to qae.
Many arguments are important to how qae parses the output
and some are added programmatically, like those for search type,
case sensitivity, and context.

However, some arguments do not fall into either of those cases,
and they can be added here.  Things like `--search-zip' to search
compressed files, or `--follow' to follow symlinks.

Sometimes settings in your config file can cause problems, which
is why `--no-config' is included here by default."
  :type '(repeat string)
  :group 'qae)

(defun qae--arguments (search-term search-type case context)
  "Return a list of command line arguments that we can execute in a shell
to obtain ripgrep results."
  ;; We put the extra arguments first so that later arguments will
  ;; override them, preventing a user from accidentally breaking
  ;; ripgrep by specifying --heading, for example.
  (let ((args (copy-sequence qae-extra-arguments)))
    (push "--color=ansi" args)
    (push "--line-number" args)
    (push "--no-heading" args)
    (push "--no-column" args)
    (push "--with-filename" args)

    (cond
     ((eq search-type 'string)
      (push "--fixed-strings" args))
     ((eq search-type 'words)
      (push "--fixed-strings" args)
      (push "--word-regexp" args))
     ((eq search-type 'regexp))
     (t
      (error "Unknown search type: %s" search-type)))

    (cond
     ((eq case 'smart)
      (push "--smart-case" args))
     ((eq case 'sensitive)
      (push "--case-sensitive" args))
     ((eq case 'ignore)
      (push "--ignore-case" args))
     (t
      (error "Unknown case: %s" case)))

    (cond
     ((eq qae--file-type 'all))
     ((eq (car-safe qae--file-type) 'type)
      (push (format "--type=%s" (cdr qae--file-type)) args))
     ((eq (car-safe qae--file-type) 'glob)
      (push (format "--glob=%s" (cdr qae--file-type)) args))
     (t
      (error "Unknown file-type: %S" qae--file-type)))

    (when context
      (push (format "--before-context=%s" (car context)) args)
      (push (format "--after-context=%s" (cdr context)) args))

    (unless qae--skip-if-hidden
      (push "--hidden" args))
    (if qae--skip-if-vcs-ignore
        ;; By default, ripgrep searches .git even when it's respecting
        ;; .gitignore, if --hidden is set. Ignore .git when we're
        ;; using .gitignore.
        ;;
        ;; https://github.com/BurntSushi/ripgrep/issues/713
        (push "--glob=!/.git" args)
      (push "--no-ignore-vcs" args))

    (push "--" args)
    (push search-term args)
    (push "." args)

    (nreverse args)))

(defun qae--write-heading ()
  "Write the qae heading with buttons reflecting the current
search settings."
  (let ((start-pos (point))
        (inhibit-read-only t))
    (insert (propertize "Search term: "
                        'face 'qae-meta-face)
            (if (eq qae--search-type 'regexp)
                (qae--propertize-regexp qae--search-term)
              (propertize
               qae--search-term
               'face 'qae-search-term-face))
            " "
            (qae--button "change" 'qae-search-term)
            "\n"
            (propertize "Search type: "
                        'face 'qae-meta-face)

            (if (eq qae--search-type 'string)
                "string"
              (qae--button "string" 'qae-type
                                'search-type 'string))
            " "
            (if (eq qae--search-type 'words)
                "words"
              (qae--button "words" 'qae-type
                                'search-type 'words))
            " "
            (if (eq qae--search-type 'regexp)
                "regexp"
              (qae--button "regexp" 'qae-type
                                'search-type 'regexp))
            "\n"
            (propertize "Case: "
                        'face 'qae-meta-face)
            (if (eq qae--search-case 'smart)
                "smart"
              (qae--button "smart" 'qae-case
                                'case 'smart))
            " "
            (if (eq qae--search-case 'sensitive)
                "sensitive"
              (qae--button "sensitive" 'qae-case
                                'case 'sensitive))
            " "
            (if (eq qae--search-case 'ignore)
                "ignore"
              (qae--button "ignore" 'qae-case
                                'case 'ignore))
            "\n"
            (propertize "Context: "
                        'face 'qae-meta-face)
            (if qae--context
                (qae--button "none" 'qae-context
                                  'context nil)
              "none")
            " "
            (qae--button "before" 'qae-context
                              'context 'before)
            (if qae--context
                (format ":%d" (car qae--context))
              "")
            " "
            (qae--button "after" 'qae-context
                              'context 'after)
            (if qae--context
                (format ":%d" (cdr qae--context))
              "")

            "\n\n"
            (propertize "Directory: "
                        'face 'qae-meta-face)
            (qae--button
             (abbreviate-file-name default-directory)
             'qae-directory)
            (if (get-text-property 0 'qae-overridden default-directory)
                (propertize " (from override)" 'face 'qae-meta-face)
              "")
            "\n"
            (propertize "Files: "
                        'face 'qae-meta-face)
            (if (eq qae--file-type 'all)
                "all"
              (qae--button "all" 'qae-file-type
                                'file-type 'all))
            " "
            (qae--button "type" 'qae-file-type
                              'file-type 'type)
            (if (eq (car-safe qae--file-type) 'type)
                (format ":%s" (cdr qae--file-type))
              "")
            " "
            (qae--button "glob" 'qae-file-type
                              'file-type 'glob)
            (if (eq (car-safe qae--file-type) 'glob)
                (format ":%s" (cdr qae--file-type))
              "")
            "\n"
            (propertize "Skip: "
                        'face 'qae-meta-face)
            (qae--button "dotfiles" 'qae-skip-hidden-type)
            (if qae--skip-if-hidden ":yes" ":no")
            " "
            (qae--button ".gitignore items" 'qae-vcs-skip-type)
            (if qae--skip-if-vcs-ignore ":yes" ":no")
            "\n\n")
    (put-text-property
     start-pos (point)
     'read-only t)
    (put-text-property
     start-pos (point)
     'front-sticky t)))

;; TODO: could we do this in the minibuffer too?
(defun qae--propertize-regexp (regexp)
  "Given a string REGEXP representing a search term with regular
expression syntax, highlight the metacharacters.
Returns a copy of REGEXP with properties set."
  (setq regexp (copy-sequence regexp))

  ;; See https://docs.rs/regex/1.0.0/regex/#syntax
  (let ((metachars
         ;; Characters that don't match themselves.
         '(?\( ?\) ?\[ ?\] ?\{ ?\} ?| ?. ?+ ?* ?? ?^ ?$))
        ;; Characters that have special regexp meaning when preceded
        ;; with a backslash. This includes things like \b but not
        ;; things like \n.
        (escape-metachars
         '(?A ?b ?B ?d ?D ?p ?s ?S ?w ?W ?z))
        (prev-char nil))
    ;; Put the standard search term face on every character
    ;; individually.
    (dotimes (i (length regexp))
      (put-text-property
       i (1+ i)
       'face 'qae-search-term-face
       regexp))
    ;; Put the metacharacter face on any character that isn't treated
    ;; literally.
    (--each-indexed (string-to-list regexp)
      (cond
       ;; Highlight everything between { and }.
       ((and (eq it ?\{) (not (equal prev-char ?\\)))
        (let ((closing-pos it-index))
          ;; TODO: we have loops like this in several places, factor
          ;; out.
          (while (and (< closing-pos (length regexp))
                      (not (eq (elt regexp closing-pos)
                               ?\})))
            (cl-incf closing-pos))
          ;; Step over the closing }, if we found one.
          (unless (= closing-pos (length regexp))
            (cl-incf closing-pos))
          (put-text-property
           it-index closing-pos
           'face
           'qae-regexp-metachar-face
           regexp)))
       ;; Highlight individual metachars.
       ((and (memq it metachars) (not (equal prev-char ?\\)))
        (put-text-property
         it-index (1+ it-index)
         'face
         'qae-regexp-metachar-face
         regexp))
       ((and (memq it escape-metachars) (equal prev-char ?\\))
        (put-text-property
         (1- it-index) (1+ it-index)
         'face 'qae-regexp-metachar-face
         regexp)))

      (setq prev-char it)))
  regexp)

(defun qae--buffer-name (search-term directory)
  (format "*qae %s %s*"
          (s-truncate 30 search-term)
          (abbreviate-file-name directory)))

(defun qae--buffers ()
  "All the current qae results buffers.
Returns a list ordered by the most recently accessed."
  (--filter (with-current-buffer it
              (eq major-mode 'qae-mode))
            ;; `buffer-list' seems to be ordered by most recently
            ;; visited first.
            (buffer-list)))

(defun qae--buffer (search-term directory initial-filename)
  "Create and initialise a search results buffer."
  (let* ((buf-name (qae--buffer-name search-term directory))
         (buf (get-buffer buf-name)))
    (if buf
        ;; There was already a buffer with this name. Reset its search
        ;; state.
        (with-current-buffer buf
          (qae--stop-and-reset))
      ;; We need to create the buffer, ensure we don't exceed
      ;; `qae-max-buffers' by killing the least recently used.
      (progn
        (when (numberp qae-max-buffers)
          (let* ((excess-buffers (-drop (1- qae-max-buffers)
                                        (qae--buffers))))
            ;; Kill buffers so we have one buffer less than the maximum
            ;; before we create a new one.
            (-each excess-buffers #'kill-buffer)))

        (setq buf (get-buffer-create buf-name))))

    (with-current-buffer buf
      (setq default-directory directory)
      (let ((inhibit-read-only t))
        ;; This needs to happen first, as it clobbers all buffer-local
        ;; variables.
        (qae-mode)
        (erase-buffer)

        (setq qae--search-term search-term)
        (setq qae--current-file nil)
        (setq qae--initial-filename initial-filename))
      (setq buffer-read-only t))
    buf))

(defun qae-cycle-files ()
  "Cycle which files are searched (all / type / glob) and restart the search."
  (interactive)
  (cond
   ((eq qae--file-type 'all)
    (setq qae--file-type
          (cons 'type (qae--read-file-type qae--initial-filename))))
   ((eq (car-safe qae--file-type) 'type)
    (setq qae--file-type (cons 'glob (qae--read-file-glob))))
   ((eq (car-safe qae--file-type) 'glob)
    (setq qae--file-type 'all)))
  (qae-restart))

(defun qae-cycle-search-type ()
  "Cycle the search type (string / words / regexp) and restart the search."
  (interactive)
  (cond
   ((eq qae--search-type 'string) (setq qae--search-type 'words))
   ((eq qae--search-type 'words) (setq qae--search-type 'regexp))
   ((eq qae--search-type 'regexp) (setq qae--search-type 'string)))
  (qae-restart))

(defun qae-cycle-search-case ()
  "Cycle the search case (smart / sensitive / ignore) and restart the search."
  (interactive)
  (cond
   ((eq qae--search-case 'smart) (setq qae--search-case 'sensitive))
   ((eq qae--search-case 'sensitive) (setq qae--search-case 'ignore))
   ((eq qae--search-case 'ignore) (setq qae--search-case 'smart)))
  (qae-restart))

(defvar qae-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "RET") #'qae-visit-result)
    (define-key map (kbd "o") #'qae-visit-result-other-window)
    ;; TODO: we should still be able to click on buttons.

    (define-key map (kbd "S") #'qae-search-term)
    (define-key map (kbd "T") #'qae-cycle-search-type)
    (define-key map (kbd "C") #'qae-cycle-search-case)
    (define-key map (kbd "F") #'qae-cycle-files)
    (define-key map (kbd "D") #'qae-directory)
    (define-key map (kbd "^") #'qae-parent-directory)
    (define-key map (kbd "g") #'qae-restart)
    (define-key map (kbd "I") #'qae-incremental)

    ;; TODO: this should work when point is anywhere in the file, not
    ;; just on its heading.
    (define-key map (kbd "TAB") #'qae-toggle-file-results)
    (define-key map (kbd "S-C-i") #'qae-toggle-all-file-results)
    (define-key map (kbd "<backtab>") #'qae-toggle-all-file-results)

    ;; Keybinding chosen to match `kill-compilation'.
    (define-key map (kbd "C-c C-k") #'qae-kill-process)

    ;; Keybinding chosen to match other tools, including `read-only-mode'.
    (define-key map (kbd "C-x C-q") #'qae-edit-mode)

    (define-key map (kbd "n") #'qae-forward-match)
    (define-key map (kbd "p") #'qae-backward-match)
    (define-key map (kbd "M-n") #'qae-forward-filename)
    (define-key map (kbd "M-p") #'qae-backward-filename)

    map)
  "Keymap for `qae-mode'.")

(defvar qae-edit-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "RET") #'qae-visit-result)
    (define-key map (kbd "C-c C-c") #'qae-mode) ;; exit edit mode
    map)
  "Keymap for `qae-edit-mode'.")

(define-derived-mode qae-mode special-mode
  '(:eval (qae--mode-line))
  "Major mode for qae results buffers."
  (remove-hook 'after-change-functions #'qae--propagate-change t))

(defun qae--find-file (path)
  "Open PATH in a buffer, and return a cons cell
\(BUF . OPENED). OPENED is nil if there was already a buffer for
this path."
  (let* ((initial-buffers (buffer-list))
         (opened nil)
         ;; Skip running find-file-hook since it may prompt the user.
         (find-file-hook nil)
         ;; If we end up opening a buffer, don't bother with file
         ;; variables. It prompts the user, and we discard the buffer
         ;; afterwards anyway.
         (enable-local-variables nil)
         ;; Bind `auto-mode-alist' to nil, so we open the buffer in
         ;; `fundamental-mode' if it isn't already open.
         (auto-mode-alist nil)
         ;; Use `find-file-noselect' so we still decode bytes from the
         ;; underlying file.
         (buf (find-file-noselect path)))
    (unless (-contains-p initial-buffers buf)
      (setq opened t))
    (cons buf opened)))

(defun qae--propagate-change (beg end length)
  "Repeat the last modification to the results buffer in the
underlying file."
  ;; We should never be called outside an edit buffer, but be
  ;; defensive. Buggy functions in change hooks are painful.
  (when (eq major-mode 'qae-edit-mode)
    (save-mark-and-excursion
      (goto-char beg)
      (-let* ((column (+ (or (qae--current-column) 0)
                         length))
              (filename (qae--filename))
              (line-number (qae--line-number))
              ((buf . opened) (qae--find-file filename))
              (inserted (buffer-substring beg end)))
        (with-current-buffer buf
          (save-mark-and-excursion
            (save-restriction
              (widen)
              (goto-char
               (qae--buffer-position line-number column))
              (delete-char (- length))
              (insert inserted)))
          ;; If we weren't visiting this file before, just save it and
          ;; close it.
          (when opened
            (basic-save-buffer)
            (kill-buffer buf)))))))

(defcustom qae-edit-mode-hook nil
  "Called after `qae-edit-mode' is turned on."
  :type 'hook
  :group 'qae)

(defun qae-edit-mode ()
  "Major mode for editing the results files directly from a
qae results buffer.

\\{qae-edit-mode-map}"
  (interactive)
  (unless (eq major-mode 'qae-mode)
    (user-error "qae-edit-mode only works in qae result buffers"))
  (when qae--running
    (user-error "Can't edit a results buffer until the search is finished"))
  ;; We deliberately don't use `define-derived-mode' here because we
  ;; want to check the previous value of `major-mode'. Initialise the
  ;; major mode manually.
  (delay-mode-hooks
    (kill-all-local-variables)
    (setq major-mode 'qae-edit-mode)
    (setq mode-name
          '(:propertize "Qae:Edit" face mode-line-emphasis))
    (use-local-map qae-edit-mode-map)
    ;; Done major mode manual initialise (copied from `define-derived-mode').

    ;; Allow editing, and propagate changes.
    (setq buffer-read-only nil)
    (add-hook 'after-change-functions #'qae--propagate-change nil t)

    (message "Now editing, use `M-x qae-mode' when finished"))

  (run-mode-hooks 'qae-edit-mode-hook))

(defun qae--current-column ()
  "When point is on a result in a results buffer, return the column offset
of the underlying file. Treats tabs as 1.

foo.el
123 h|ello world

In this example, the column is 1."
  (let* ((line-start (line-beginning-position))
         (line-number
          (get-text-property line-start 'qae-line-number))
         (line-number-width
          (max qae--position-column-width
               (length (number-to-string line-number))))
         (char-count 0))
    (save-excursion
      (while (not (equal (point) line-start))
        (cl-incf char-count)
        (backward-char 1)))
    (if (< char-count line-number-width)
        nil
      (- char-count line-number-width))))

(defun qae--flash-column-offsets (start end)
  "Temporarily highlight column offset from START to END."
  (let* ((line-start (line-beginning-position))
         (overlay (make-overlay
                   (+ line-start start)
                   (+ line-start end))))
    (overlay-put overlay 'face 'highlight)
    (run-with-timer 1.5 nil 'delete-overlay overlay)))

(defun qae--match-face-p (pos)
  "Is there a match face at POS?"
  (eq (get-text-property pos 'face) 'qae-match-face))

(defun qae--match-positions ()
  "Return a list of column offsets of the current line's matches.
Each item in the list has the form (START-OFFSET END-OFFSET)."
  (let (positions)
    (save-excursion
      (beginning-of-line)

      (let* ((line-number
              (get-text-property (point) 'qae-line-number))
             (line-number-width
              (max qae--position-column-width
                   (length (number-to-string line-number))))
             (i 0)
             (start-pos 0)
             (line-end-pos (line-end-position)))

        (forward-char line-number-width)

        (while (<= (point) line-end-pos)
          ;; If we've just entered a match, record the start position.
          (when (and (qae--match-face-p (point))
                     (not (qae--match-face-p (1- (point)))))
            (setq start-pos i))
          ;; If we've just left a match, record the match range.
          (when (and (not (qae--match-face-p (point)))
                     (qae--match-face-p (1- (point))))
            (push (list start-pos i) positions))

          (setq i (1+ i))
          (forward-char 1))))

    (nreverse positions)))

(defun qae--buffer-position (line-number column-offset)
  "Calculate the buffer position that corresponds to LINE-NUMBER at COLUMN-OFFSET
in the current buffer."
  (save-excursion
    (save-restriction
      (widen)
      (goto-char (point-min))
      (forward-line (1- line-number))
      (forward-char column-offset)

      (point))))

(defun qae--filename (&optional pos)
  "Get the filename of the result at point POS.
If POS is nil, use the beginning position of the current line."
  (get-text-property (or pos (line-beginning-position)) 'qae-filename))

(defun qae--line-number ()
  "Get the filename of the result at point."
  (get-text-property (line-beginning-position) 'qae-line-number))

(defun qae--visit-result (open-fn)
  "Goto the search result at point."
  (interactive)
  (let* ((pos (line-beginning-position))
         (file-name (qae--filename))
         (line-number (qae--line-number))
         (column-offset (when line-number (qae--current-column)))
         (match-positions (when line-number (qae--match-positions))))
    (when file-name
      (when overlay-arrow-position
        (set-marker overlay-arrow-position nil))
      ;; Show an arrow next to the last result viewed. This is
      ;; consistent with `compilation-next-error-function' and also
      ;; useful with `qae-visit-result-other-window'.
      (setq overlay-arrow-position (copy-marker pos))

      (funcall open-fn file-name)
      (goto-char (point-min))

      (when line-number
        ;; If point was on the line number rather than a specific
        ;; position on the line, go the first match. This is generally
        ;; what users want, especially when there are long lines.
        (unless column-offset
          (if-let (first-match-pos (car match-positions))
              (setq column-offset (car first-match-pos))
            (setq column-offset 0)))

        (-let [destination-pos (qae--buffer-position
                                line-number column-offset)]
          ;; Put point on the position of the match, widening the
          ;; buffer if necessary.
          (when (or (< destination-pos (point-min))
                    (> destination-pos (point-max)))
            (widen))
          (goto-char destination-pos)

          ;; Temporarily highlight the parts of the line that matched
          ;; the search term.
          (-each match-positions
            (-lambda ((start end))
              (qae--flash-column-offsets start end))))))))

(defun qae-visit-result-other-window ()
  "Goto the search result at point, opening in another window."
  (interactive)
  (qae--visit-result #'find-file-other-window))

(defun qae-visit-result ()
  "Goto the search result at point."
  (interactive)
  (qae--visit-result #'find-file))

(defvar-local qae--hidden-files nil
  "An alist recording which files currently have their lines
hidden in this qae results buffer.

Keys are interned filenames, so they compare with `eq'.")

(defun qae-toggle-file-results ()
  "Show/hide the results of the file at point."
  (interactive)
  (let* ((file-name (qae--filename))
         (line-number (qae--line-number)))
    (when (and file-name (not line-number))
      ;; We're on a file heading.
      (if (alist-get (intern file-name) qae--hidden-files)
          (qae--show)
        (qae--hide)))))

(defun qae-toggle-all-file-results ()
  "Show/hide the results of all files."
  (interactive)
  (let ((should-show (cl-some #'cdr qae--hidden-files))
        (seen-files nil))
    (save-excursion
      (goto-char (point-min))
      (while (not (eq (point) (point-max)))
        (goto-char (or (next-single-property-change (point) 'qae-filename)
                       (point-max)))
        (when (and (qae--filename)
                   (not (member (qae--filename) seen-files)))
          (push (qae--filename) seen-files)
          (if should-show
              (qae--show)
            (qae--hide)))))))

(defun qae--show ()
  (-let* ((file-name (qae--filename))
          ((start-pos end-pos) (alist-get (intern file-name) qae--hidden-files)))
    (remove-overlays start-pos end-pos 'invisible t)
    (setf (alist-get (intern file-name) qae--hidden-files)
          nil)))

(defun qae--hide ()
  "Hide the file results immediately after point."
  (save-excursion
    (let* ((file-name (qae--filename))
           (start-pos
            (progn
              (forward-line)
              (point)))
           (end-pos
            (progn
              (while (and
                      (or (get-text-property (point) 'qae-line-number)
                          (get-text-property (point) 'qae-separator))
                      (not (bobp)))
                (forward-line))
              ;; Step over the newline.
              (1+ (point))))
           (o (make-overlay start-pos end-pos)))
      (overlay-put o 'invisible t)
      (setf (alist-get (intern file-name) qae--hidden-files)
            (list start-pos end-pos)))))

(defun qae--interrupt-process ()
  "Gracefully stop the rg process, synchronously."
  (-when-let (proc (get-buffer-process (current-buffer)))
    ;; Ensure that our process filter is not called again.
    (set-process-filter proc #'ignore)

    (interrupt-process proc)
    ;; Wait for the process to terminate, so we know that
    ;; `qae--process-sentinel' has been called.
    (while (process-live-p proc)
      ;; `redisplay' can trigger process filters or sentinels.
      (redisplay)
      (sleep-for 0.1))))

(defun qae-kill-process ()
  "Kill the qae process associated with the current buffer."
  (interactive)
  (if (get-buffer-process (current-buffer))
      (qae--interrupt-process)
    (message "No process running.")))

(defun qae--item-p (pos)
  "Is there something at POS that we can interact with?"
  (or (button-at pos)
      (qae--filename pos)))

(defun qae--filename-p (pos)
  "Is there a filename at POS that we can interact with?"
  (eq (get-text-property pos 'face) 'qae-filename-face))

(defun qae--move (forward-p)
  "Move to the next item.
This will either be a button, a filename, or a search result."
  (interactive)
  (let ((pos (point)))
    ;; If point is initially on an item, move past it.
    (while (and (qae--item-p pos)
                (if forward-p
                    (< pos (point-max))
                  (> pos (point-min))))
      (if forward-p
          (cl-incf pos)
        (cl-decf pos)))
    ;; Find the next item.
    (while (and (not (qae--item-p pos))
                (if forward-p
                    (< pos (point-max))
                  (> pos (point-min))))
      (if forward-p
          (cl-incf pos)
        (cl-decf pos)))
    ;; Regardless of direction, ensure point is at the beginning of
    ;; the item.
    (while (and (if forward-p
                    (< pos (point-max))
                  (> pos (point-min)))
                (qae--item-p (1- pos)))
      (cl-decf pos))
    ;; If we reached an item (we aren't at the first/last item), then
    ;; go to it.
    (when (qae--item-p pos)
      (goto-char pos))))

(defun qae-forward ()
  "Move forward to the next item.
This will either be a button, a filename, or a search result. See
also `qae-forward-match'."
  (interactive)
  (qae--move t))

(defun qae-backward ()
  "Move backward to the previous item.
This will either be a button, a filename, or a search result. See
also `qae-backward-match'."
  (interactive)
  (qae--move nil))

(defun qae-forward-filename ()
  "Move forward to the next filename."
  (interactive)
  (qae--move-match t 'qae-filename-face))

(defun qae-backward-filename ()
  "Move backward to the previous filename."
  (interactive)
  (qae--move-match nil 'qae-filename-face))

(defun qae--move-match (forward-p face)
  "Move point to the beginning of the next/previous match."
  (interactive)
  (let ((start-pos (point)))
    ;; Move over the current match, if we were already on one.
    (while (eq (get-text-property (point) 'face)
               face)
      (if forward-p (forward-char) (backward-char)))
    (condition-case nil
        (progn
          ;; Move point to the next match, which may be on the same line.
          (while (not (eq (get-text-property (point) 'face)
                          face))
            (if forward-p (forward-char) (backward-char)))
          ;; Ensure point is at the beginning of the match.
          (unless forward-p
            (while (eq (get-text-property (point) 'face)
                       face)
              (backward-char))
            (forward-char)))
      ;; Don't move point beyond the last match. However, it's still
      ;; useful to signal that we're at the end, so users can use this
      ;; command with macros and terminate when it's done.
      (beginning-of-buffer
       (goto-char start-pos)
       (signal 'beginning-of-buffer nil))
      (end-of-buffer
       (goto-char start-pos)
       (signal 'end-of-buffer nil)))))

(defun qae-forward-match ()
  "Move point forward to the beginning of next match.
Note that a result line may contain more than one match, or zero
matches (if the result line has been truncated)."
  (interactive)
  (qae--move-match t 'qae-match-face))

(defun qae-backward-match ()
  "Move point backward to the beginning of previous match."
  (interactive)
  (qae--move-match nil 'qae-match-face))

(defun qae--start (search-term search-type case)
  "Start a ripgrep search."
  (setq qae--spinner (spinner-create 'progress-bar t))
  (setq qae--running t)
  (setq qae--result-count 0)
  (spinner-start qae--spinner)
  (let* ((args (qae--arguments
                search-term search-type case
                qae--context))
         (process
          (apply #'start-file-process
                 (format "qae %s" search-term)
                 (current-buffer)
                 qae-executable
                 args)))
    (setq qae--debug-command
          (format "%s %s" qae-executable (s-join " " args)))
    (set-process-filter process #'qae--process-filter)
    (set-process-sentinel process #'qae--process-sentinel)))

(defun qae--stop-and-reset ()
  "Terminate the current search and reset any search state."
  ;; Stop the old search, so we don't carry on inserting results from
  ;; the last thing we searched for.
  (qae--interrupt-process)

  (let ((inhibit-read-only t))
    ;; Reset UI: remove results, reset items hidden by TAB, and arrow
    ;; position.
    (erase-buffer)
    (setq qae--hidden-files nil)
    (when overlay-arrow-position
      (set-marker overlay-arrow-position nil))

    ;; Reset intermediate search state.
    (setq qae--current-file nil)
    (setq qae--spinner nil)
    (setq qae--remaining-output nil)
    (setq qae--current-file nil)
    (setq qae--debug-first-output nil)
    (setq qae--imenu-alist nil)))

(defun qae-restart ()
  "Re-run ripgrep with the current search settings."
  (interactive)
  ;; If we haven't started yet, start the search if we've been called
  ;; by the user.
  (when (and qae--postpone-start
             (called-interactively-p 'interactive))
    (setq qae--postpone-start nil))

  (qae--stop-and-reset)

  (let ((start-point (point))
        (inhibit-read-only t))
    (qae--write-heading)
    ;; If the point was in the heading, ensure that we restore its
    ;; position.
    (goto-char (min (point-max) start-point))

    (if qae--postpone-start
        (qae--write-postponed)
      (qae--start
       qae--search-term
       qae--search-type
       qae--search-case))))

(defun qae--read-search-term ()
  "Read a search term from the minibuffer.
If region is active, return that immediately.  Otherwise, prompt
for a string, offering the current word as a default."
  (let (search-term)
    (if (use-region-p)
        (progn
          (setq search-term
                (buffer-substring-no-properties (region-beginning) (region-end)))
          (deactivate-mark))
      (let* ((sym (symbol-at-point))
             (sym-name (when sym
                         (substring-no-properties (symbol-name sym))))
             (prompt
              (qae--search-prompt sym-name)))
        (setq search-term
              (read-from-minibuffer
               prompt nil nil nil 'qae-history sym-name))
        (when (equal search-term "")
          (setq search-term sym-name))))
    (unless (equal (car qae-history) search-term)
      (push search-term qae-history))
    search-term))

(defun qae-incremental ()
  (interactive)
  (catch 'break
    (let ((qae--incremental-active t)
          (search-term (or qae--search-term "")))
      (while t
        (let ((next-char
               (read-char
                ;; TODO: Use the same prompt format as other search options.
                (format "%s %s"
                        (apply #'propertize "Incremental Search (RET when done):" minibuffer-prompt-properties)
                        search-term))))
          (cond
           ((eq next-char ?\C-m)
            (throw 'break nil))
           ((eq next-char ?\C-?)
            (setq search-term (s-left -1 search-term)))
           (t
            (setq search-term (concat search-term (list next-char))))))
        (when (> (length search-term) 2)
          (setq qae--search-term search-term)
          (qae-restart))))))


(defun qae--normalise-dirname (path)
  "Expand PATH and ensure that it doesn't end with a slash.
If PATH is remote path, it is not expanded."
  (directory-file-name (if (file-remote-p path)
                           path
                         (let (file-name-handler-alist)
                           (expand-file-name path)))))

(defun qae--lookup-override (path)
  "If PATH is present in `qae-project-root-overrides',
return the overridden value.
Otherwise, return PATH as is."
  (let* ((normalised-path (qae--normalise-dirname path))
         (override
          (-first
           (-lambda ((original . _))
             (equal (qae--normalise-dirname original) normalised-path))
           qae-project-root-overrides)))
    (when override
      (setq path (cdr override))
      (unless (stringp path)
        (user-error "Bad override: expected a path string, but got: %S" path))
      (setq path (propertize path 'qae-overridden t)))
    path))

(defun qae--project-root ()
  "Guess the project root of the given FILE-PATH."
  (let ((root default-directory)
        (project (project-current)))
    (when project
      (cond ((and (fboundp 'project-root)
                  (>= emacs-major-version 28))
             ;; `project-root' was added in Emacs 28, but projectile
             ;; defines it unconditionally.
             (setq root (project-root project)))
            (t
             ;; Older Emacsen.
             (-when-let (roots (project-roots project))
               (setq root (car roots))))))
    (when root
      (qae--lookup-override root))))

(defun qae--write-postponed ()
  "Write a message to the current buffer informing the user that
qae is ready but not yet searching."
  (let* ((inhibit-read-only t)
         (restart-key
          (where-is-internal #'qae-restart qae-mode-map t)))
    (save-excursion
      (goto-char (point-max))
      (insert
       (format "Press %s to start the search."
               (key-description restart-key))))))

(defun qae--mode-line ()
  (let* ((s (if qae--result-count
                (format "Qae:%s" qae--result-count)
              "Qae"))
         (spinner-str (spinner-print qae--spinner)))
    (if spinner-str
        (concat s " " spinner-str)
      s)))

(defun qae--create-imenu-index ()
  "Create `imenu' index for matched files."
  (when qae--imenu-alist
    (list (cons "Files" (reverse qae--imenu-alist)))))

;;;###autoload
(defun qae (search-term &optional directory)
  "Start a ripgrep search for SEARCH-TERM in DIRECTORY.

If not provided, DIR defaults to the directory as determined by
`qae-project-root-function'.

See also `qae-project-root-overrides'.

If called with a prefix argument, create the results buffer but
don't actually start the search."
  (interactive (list (qae--read-search-term)))
  (let* ((dir (or directory
                  (funcall qae-project-root-function)))
         (buf (qae--buffer
               search-term
               dir
               (or qae--initial-filename
                   (buffer-file-name))))
         (last-results-buf (car-safe (qae--buffers)))
         prev-search-type
         prev-search-case
         prev-skip-if-hidden
         prev-skip-if-vcs-ignore)
    ;; Find out what search settings were used last time.
    (when last-results-buf
      (with-current-buffer last-results-buf
        (setq prev-search-type qae--search-type)
        (setq prev-search-case qae--search-case)
        (setq prev-skip-if-hidden qae--skip-if-hidden)
        (setq prev-skip-if-vcs-ignore qae--skip-if-vcs-ignore)))

    (funcall qae-display-buffer-function buf)

    (with-current-buffer buf
      (setq imenu-create-index-function #'qae--create-imenu-index)
      (setq next-error-function #'qae-next-error)
      (setq next-error-last-buffer buf)

      ;; If we have previous search settings, apply them to our new
      ;; search results buffer.
      (when last-results-buf
        (setq qae--search-type prev-search-type)
        (setq qae--search-case prev-search-case)
        (setq qae--skip-if-hidden prev-skip-if-hidden)
        (setq qae--skip-if-vcs-ignore prev-skip-if-vcs-ignore))

      (qae--write-heading)

      (if current-prefix-arg
          ;; Don't start the search, just create the buffer and inform
          ;; the user how to start when they're ready.
          (progn
            (setq qae--postpone-start t)
            (qae--write-postponed))
        ;; Start the search immediately.
        (qae--start
         search-term
         qae--search-type
         qae--search-case)))))

(defun qae-next-error (arg reset)
  "Move to the next error.
If ARG is given, move by that many errors.

This is intended for use with `next-error-function', which see."
  (when reset
    (goto-char (point-min)))
  (beginning-of-line)
  (let ((direction (> arg 0)))
    (setq arg (abs arg))

    (while (and
            (not (zerop arg))
            (not (eobp)))
      (if direction
          (forward-line 1)
        (forward-line -1))
      ;; If we are on a specific result (not a heading), we have a line
      ;; number.
      (when (get-text-property (point) 'qae-line-number)
        (cl-decf arg))))
  (qae-visit-result-other-window))

(defun qae-debug ()
  "Show a buffer with some debug information about the current search."
  (interactive)
  (unless (eq major-mode 'qae-mode)
    (user-error "qae-debug should be run in a qae results buffer"))

  (let ((command qae--debug-command)
        (output qae--debug-first-output)
        (buf (get-buffer-create "*qae debug*"))
        (inhibit-read-only t))
    (pop-to-buffer buf)
    (erase-buffer)
    (special-mode)
    (setq buffer-read-only t)

    (insert
     (propertize
      "About your environment:\n"
      'face 'qae-filename-face)
     (format "Platform: %s\n" system-type)
     (format "Emacs version: %s\n" emacs-version)
     (format "Command: %s\n" command)
     (format "default-directory: %S\n" default-directory)
     (format "exec-path: %S\n" exec-path)
     (if (boundp 'tramp-remote-path)
     (format "tramp-remote-path: %S\n" tramp-remote-path)
       "")
     (propertize
      "\nInitial output from ripgrep:\n"
      'face 'qae-filename-face)
     (format "%S" output)
     (propertize
      "\n\nPlease file bugs at https://github.com/Wilfred/qae/issues/new"
      'face 'qae-filename-face))))

(defun qae-kill-all-buffers ()
  "Kill all open qae buffers."
  (interactive)
  (dolist (buffer (qae--buffers))
    (kill-buffer buffer)))

(provide 'qae)
;;; qae.el ends here

;; Local Variables:
;; byte-compile-warnings: (not obsolete)
;; End:
