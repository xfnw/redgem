# redgem
a zipapp gemini server

## using
redgem can be built with cargo
```
cargo build -r
```

prepare a zip of the files you want to serve. then, concatenate it with
the redgem binary and fix it up
```
echo hello world > index.gmi
zip files.zip index.gmi
cat target/release/redgem files.zip > redgem.zip
chmod +x redgem.zip
zip -A redgem.zip
```

to run it you'll need a tls certificate, a simple self-signed one can
be created with with openssl
```
openssl req -x509 -newkey rsa:4096 -days 3650 -nodes -keyout gemini.pem -out gemini.pem -subj "/CN=example.com"
```

then run the zip file while passing it the certificate
```
./redgem.zip gemini.pem
```

you can modify the contents later like any other zip file, but try not
to do that while redgem is running. the zip library it uses re-opens the
zip file when reading for concurrency reasons, and it'll get confused
if the contents are different
