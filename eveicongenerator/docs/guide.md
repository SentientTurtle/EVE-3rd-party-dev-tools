# EVE Icon Generator guide

### Global options

* `--cache-folder <directory>`, `-c <directory>` (default: `./cache`)  
  Folder for game file cache.  
  WARNING: All other files in this folder will be deleted during cleanup  
  This folder should persist between runs to avoid re-downloading files from CCP servers.
* `--icon-folder <directory>`, `-i <directory>` (default: `./icons`)  
  Folder for storing built icons.
  This folder may be persisted to cache image-compositing work.
  NOTE: Other files in this folder will not be deleted. Only unnecessary files created by the previous run of this program will be cleaned up.
* `--logfile <file>`, `-l <file>` 
  Log file destination, if unset no logging is performed.  
  Log contains detailed information on icon generation & written output, and is several megabytes of text.
* `--append_log`
  If set, appends to the specified logfile. If omitted, truncates log file. Requires `--logfile`.
* `--silent`  
  Silent mode, implied by `checksum` output mode if no checksum file is specified.
* `--data <SDE, FSD>`, `-d <SDE, FSD>`
  Data source to use, SDE downloads the Static Data Export, FSD requires a *windows* python2 to be available.
  NOTE: 'FSD' mode is an optional feature that must be enabled when compiling this program, and may not be available.
* `--python2 <command>`
  Command prefix for python2, required if using `FSD` data mode  
  Warning: This *MUST* be a windows-compatible install as FSD loading requires running windows-binary python libraries. On linux, use of WINE or similar tools may work.
* `--force_rebuild`, `-f`
  Force rebuilding of images, re-doing compositing of all icons. Recommended when updating the application to ensure any changes to compositing have been applied to cached icons.
* `--skip_if_fresh`, `-s`
  If no icons have changed since the last run, skip generating output.
  NOTE: Ignored for `checksum` output mode with no checksum file specified, the checksum will still be output to stdout.
* `--use_magick`
  If set, attempts to use imagemagick 7 (`magick`) for image compositing
  DEPRECATED

Output mode subcommands:
* `help [subcommand]` Displays help text for the specified subcommand
* `service_bundle`
  Generates a de-duplicated icon .zip archive, including metadata compatible with the "Image Service" routes.
  * `--out <file>` Output file for zip archive, required.
* `iec`
  Generates an 'Image Export Collection'-compatible icon .zip archive.
  * `--out <file>` Output file for zip archive, required.
* `web_dir`  
  Prepares a directory for web hosting 'image service' compatible routes by creating symlinks & metadata files, see "webmode.md".
  * `--out <directory>` Output directory to write into, required.
  * `--copy_files` Copies files rather than using symlinks.
  * `--hardlink` Use hard links rather than using soft links.
* `checksum`
  Emits a checksum of the current icon index, writes to stdout if no output file is specified.
  * `--out <file>` Output file for checksum, optional.