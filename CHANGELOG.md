# changelog

## 1.0.3 - 2025-10-08

### fixes
- requests with a query string are now rejected, since redgem does not
  support asking for input
- failure to read a request will no longer erroneously report that it
  was too long

## 1.0.2 - 2025-07-12

### fixes
- file extensions for mime type guessing are now case insensitive
- visiting an index page without a trailing slash will now redirect,
  and visiting normal files with a trailing slash is no longer
  accepted, in order to avoid breaking relative links
- errors while opening the zip are no longer eaten when daemonizing

## 1.0.1 - 2025-06-28

### changes
- response timeout increased from 5 minutes to 10 minutes

## 1.0.0 - 2025-05-30
initial release
