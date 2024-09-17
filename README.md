# helm2oci

Convert a helm chart archive to OCI layout format.

## Usage

```bash
Usage: helm2oci <chart> [--output <output>]

Convert Helm chart archive to OCI layout

Positional Arguments:
  chart             path to Helm chart archive

Options:
  --output          path to output directory. The directory is created if it
                    does not exist. Defaults to the chart name.
  --help            display usage information
```

## Installation

Build from source using `cargo build`.

## Trademarks

This project may contain trademarks or logos for projects, products, or services. Authorized use of Microsoft trademarks or logos is subject to and must follow [Microsoft’s Trademark & Brand Guidelines](https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks/usage/general). Use of Microsoft trademarks or logos in modified versions of this project must not cause confusion or imply Microsoft sponsorship. Any use of third-party trademarks or logos are subject to those third-party’s policies.
