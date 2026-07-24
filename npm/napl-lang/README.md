# napl-lang

The npm distribution of the [NAPL](https://github.com/conciv-dev/napl) toolchain — a
single native Rust binary (`napl`) shipped as per-platform packages.

```bash
npm install -g napl-lang
napl --help

# or run without installing
npx napl-lang init
```

Installing `napl-lang` pulls in exactly one matching platform package
(`@napl-lang/binary-<os>-<arch>`) through `optionalDependencies`; npm selects it from
each package's `os`/`cpu` fields. The tiny `bin/napl.js` launcher resolves that
package's binary and execs it, forwarding all arguments and the exit code. There is
no `postinstall` step.

If no prebuilt binary matches your platform, the launcher prints instructions for
installing from source (`cargo`) or via the `curl | sh` script.
