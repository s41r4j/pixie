#!/bin/sh
# Assemble the website into _site/: the pages in site/, plus the fonts, brand icons and
# CHANGELOG.md they load. Used by .github/workflows/pages.yml and `npm run site`.
set -eu
cd "$(dirname "$0")/.."
rm -rf _site
mkdir -p _site/resources/icons _site/resources/fonts
cp -R site/. _site/
cp CHANGELOG.md _site/
cp -R resources/icons/brand resources/icons/system resources/icons/pattern _site/resources/icons/
cp resources/icons/ext-32.png resources/icons/app-icon-1024.png _site/resources/icons/
cp resources/fonts/*.ttf resources/fonts/OFL-*.txt _site/resources/fonts/
touch _site/.nojekyll
echo "site → _site/"
