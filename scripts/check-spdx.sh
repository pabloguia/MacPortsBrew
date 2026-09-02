#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# Every source file carries `SPDX-License-Identifier: Apache-2.0` in its first
# lines (ADR-0001; docs/spec/08-distribution-and-project.md). Cargo manifests
# and lock files are exempt: they carry the licence in package metadata.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

bad=0
while IFS= read -r f; do
  case "$(basename "$f")" in Cargo.toml|Cargo.lock) continue ;; esac
  if ! head -5 "$f" | grep -q 'SPDX-License-Identifier: Apache-2.0'; then
    echo "spdx: missing header: $f"
    bad=1
  fi
done < <(git ls-files -- '*.rs' '*.sh' '*.py' '*.toml' 2>/dev/null; \
         git ls-files --others --exclude-standard -- '*.rs' '*.sh' '*.py' '*.toml' 2>/dev/null)
[ "$bad" = 0 ] && echo "spdx: ok"
exit "$bad"
