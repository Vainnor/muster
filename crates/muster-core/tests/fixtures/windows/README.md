Windows `.lnk` test data is built in code by `detect::windows::fixture::build`, byte by byte
from the MS-SHLLINK layout, instead of binary files copied from a real Start Menu.
`tests/detection.rs` checks the reader against a shortcut that Windows itself writes, on the
Windows CI runner.
