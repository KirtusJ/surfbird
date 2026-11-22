#!/usr/bin/env python3
"""Is CATALOG.md this repository's tree, and is it free of session context?

    python3 tests/check_catalog.py /path/to/pcp-os/build/check_catalog.py     (from anywhere)

This runs PCP OS's catalogue check on this repository. Its predicates K1 to K7, and what K6 does
and does not assure about links, are stated in that file and are not restated or changed here.
The file is given by path; the workflow takes it from pcp-os at a pinned commit and this program
refuses any other bytes. Two settings differ, and only after the check's own controls have run
with its own settings: no folder is catalogued below the top level, and no path is allowed to
hold session context. Exit 0 only if all hold. Standard library and git only.
"""
import hashlib
import importlib.util
import os
import sys

# build/check_catalog.py at pcp-os 391efc8f724b7857ab9fd6dccbf14a885e4d09ec.
PINNED = "dea22094bfa5a694c3f9d075b0fe8c4239fe344fd534610a1247ecf4de71995a"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def main():
    if len(sys.argv) != 2:
        sys.exit("give the path of pcp-os's build/check_catalog.py; nothing was checked")
    source = os.path.abspath(sys.argv[1])
    with open(source, "rb") as handle:
        found = hashlib.sha256(handle.read()).hexdigest()
    if found != PINNED:
        sys.exit("the check given is not the pinned one (SHA-256 %s); nothing was checked" % found)
    spec = importlib.util.spec_from_file_location("pcpos_check_catalog", source)
    check = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(check)
    check.selftest()                 # the check's own controls, with its own settings
    check.selftest = lambda: None    # they have run; main() would run them again with ours
    check.DEEP = ()
    check.ALLOWED = ()
    check.SELF = ()
    os.chdir(ROOT)
    check.main()


if __name__ == "__main__":
    main()
