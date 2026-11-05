#!/bin/sh
# Optional, run it yourself once: creates a self-signed code-signing certificate
# "openclicky Local Signing" in your login keychain. Builds signed with it keep the same
# identity, so macOS remembers Screen Recording / Accessibility / Microphone across rebuilds.
#   sh scripts/create-signing-identity.sh
#   APPLE_SIGNING_IDENTITY="openclicky Local Signing" npm run app:build
set -e
NAME="openclicky Local Signing"
if security find-identity -v -p codesigning | grep -q "$NAME"; then
  echo "\"$NAME\" already exists."; exit 0
fi
TMP=$(mktemp -d)
cat > "$TMP/cfg" <<CFG
[req]
distinguished_name = dn
prompt = no
[dn]
CN = $NAME
[ext]
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
basicConstraints = critical,CA:false
CFG
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 -keyout "$TMP/key.pem" -out "$TMP/cert.pem" -config "$TMP/cfg" -extensions ext
openssl pkcs12 -export -legacy -inkey "$TMP/key.pem" -in "$TMP/cert.pem" -out "$TMP/id.p12" -passout pass:openclicky
security import "$TMP/id.p12" -k ~/Library/Keychains/login.keychain-db -P openclicky -T /usr/bin/codesign
security add-trusted-cert -r trustRoot -p codeSign -k ~/Library/Keychains/login.keychain-db "$TMP/cert.pem"
rm -rf "$TMP"
echo "Created \"$NAME\". Build with: APPLE_SIGNING_IDENTITY=\"$NAME\" npm run app:build"
