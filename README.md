<div align="center">
<pre>
  ▓▓▓▓▓▓▓▓    ▓▓▓▓▓▓▓▓  ▓▓      ▓▓    ▓▓▓▓▓▓    ▓▓      ▓▓  ▓▓      ▓▓  
  ▓▓▓▓▓▓▓▓    ▓▓▓▓▓▓▓▓  ▓▓      ▓▓    ▓▓▓▓▓▓    ▓▓      ▓▓  ▓▓      ▓▓  
▓▓          ▓▓          ▓▓      ▓▓  ▓▓      ▓▓    ▓▓  ▓▓      ▓▓  ▓▓    
▓▓          ▓▓          ▓▓      ▓▓  ▓▓      ▓▓    ▓▓  ▓▓      ▓▓  ▓▓    
  ▓▓▓▓▓▓      ▓▓▓▓▓▓    ▓▓▓▓▓▓▓▓▓▓  ▓▓      ▓▓      ▓▓          ▓▓      
  ▓▓▓▓▓▓      ▓▓▓▓▓▓    ▓▓▓▓▓▓▓▓▓▓  ▓▓      ▓▓      ▓▓          ▓▓      
        ▓▓          ▓▓  ▓▓      ▓▓  ▓▓      ▓▓    ▓▓  ▓▓        ▓▓      
        ▓▓          ▓▓  ▓▓      ▓▓  ▓▓      ▓▓    ▓▓  ▓▓        ▓▓      
▓▓▓▓▓▓▓▓    ▓▓▓▓▓▓▓▓    ▓▓      ▓▓    ▓▓▓▓▓▓    ▓▓      ▓▓      ▓▓      
▓▓▓▓▓▓▓▓    ▓▓▓▓▓▓▓▓    ▓▓      ▓▓    ▓▓▓▓▓▓    ▓▓      ▓▓      ▓▓      
</pre>
</div>      


[![CI](https://github.com/Datron/sshoxy/actions/workflows/ci.yml/badge.svg)](https://github.com/Datron/sshoxy/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Datron/sshoxy?sort=semver&display_name=tag)](https://github.com/Datron/sshoxy/releases)
[![License: MIT](https://img.shields.io/github/license/Datron/sshoxy)](./LICENSE)
![Rust 1.97](https://img.shields.io/badge/rust-1.97%2B-orange?logo=rust)

--------------

sshoxy (pronounced shock-see) is a ssh server that acts as a proxy for ssh connections to a program running on the server, and only that program. It lets you build TUI/CLI applications that can be accessed over ssh like terminal.shop or blinkenlights.

# Setup

The easiest way to get started is to use the prebuilt binaries from the [releases](https://github.com/datron/sshoxy/releases) page. A helper script has been provided with this repo to install sshoxy

```bash
curl -sSL https://github.com/Datron/sshoxy/releases/download/v0.2.0/install.sh | bash
```

# Try it

No guarantees that the server stays up, but you can try it out by running the following command:

```bash
ssh -p 2222 <username>@arcade.jilebi.ai
```
