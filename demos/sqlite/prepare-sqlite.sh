#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
vendor="$root/app/packages/sqlite/vendor"
mkdir -p "$vendor"

download_page="$vendor/download.html"
curl -fsSL https://sqlite.org/download.html -o "$download_page"

python3 - "$download_page" "$vendor" <<'PY'
import re
import sys
from pathlib import Path
from urllib.parse import urljoin
from urllib.request import urlretrieve
from zipfile import ZipFile

download_page = Path(sys.argv[1])
vendor = Path(sys.argv[2])
html = download_page.read_text()

match = re.search(r'PRODUCT,[^,]+,([^,]*sqlite-amalgamation-\d+\.zip),', html)
if not match:
    match = re.search(r"d391\('[^']+','([^']*sqlite-amalgamation-\d+\.zip)'\)", html)
if not match:
    raise SystemExit("could not find sqlite-amalgamation download on sqlite.org")

url = urljoin("https://sqlite.org/download.html", match.group(1))
archive = vendor / Path(url).name
urlretrieve(url, archive)

with ZipFile(archive) as zip_file:
    for name in zip_file.namelist():
        if name.endswith("/sqlite3.c") or name.endswith("/sqlite3.h"):
            target = vendor / Path(name).name
            target.write_bytes(zip_file.read(name))
PY

cc -DSQLITE_THREADSAFE=0 -DSQLITE_OMIT_LOAD_EXTENSION -c "$vendor/sqlite3.c" -o "$vendor/sqlite3.o"
ar rcs "$vendor/libsqlite3.a" "$vendor/sqlite3.o"

echo "built $vendor/libsqlite3.a"
