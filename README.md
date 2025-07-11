# redgem
a zipapp gemini server

## building
redgem can be built with cargo
```
cargo build -r
```
optionally, cross compiling with [cross] or [zigbuild] to a musl
target and `--profile smol` can be used to get a more portable
(static) and smaller binary. there are also some features that
can be toggled, a list of them is in `Cargo.toml`

[cross]: https://github.com/cross-rs/cross
[zigbuild]: https://github.com/rust-cross/cargo-zigbuild

turning it into a zipapp is as simple as concatenating a zip of the
files you want to serve with the redgem binary, setting it executable,
and correcting the zip offsets
```
cat target/release/redgem src/tests/test.zip > redgem.zip
chmod +x redgem.zip
zip -A redgem.zip
```

## usage
to run it you'll need a tls certificate, a reasonable self-signed one
can be created with with openssl
```
openssl req -x509 -newkey rsa:4096 -days 3650 -noenc \
 -keyout gemini.pem -out gemini.pem -subj "/CN=example.com" \
 -addext "subjectAltName = DNS:example.com, IP:127.0.0.1, IP:::1" \
 -addext basicConstraints=critical,CA:FALSE
```
if you do not care about users of older tls libraries, `ed25519`
instead of `rsa:4096` gives a much nicer smaller certificate

then run the zip file while passing it the certificate
```
./redgem.zip gemini.pem
```

you can modify the contents later like any other zip file, but try not
to do that while redgem is running. the zip library it uses re-opens the
zip file when reading for concurrency reasons, and it'll get confused
if the contents are different

## upgrading
redgem can be removed from a zip file with `zip -J`. the resulting zip
can then be re-concatenated with a new version of redgem like in the
build instructions

## gempub
while redgem does not have any specific [gempub] support, the format
is simple enough that it can mostly serve the contents of a gempub.
this means you can make a gemini server gempub polygot!

however, note that redgem lacks encoding detection for gemtext, and
just serves everything as UTF-8. this will be incorrect for gempub
files that have an encoding other than UTF-8 specified

[gempub]: https://codeberg.org/oppenlab/gempub
