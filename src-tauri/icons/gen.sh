#!/bin/bash
# Generate Teacha app icons

cd "$(dirname "$0")"

# Create base 512x512 icon with blue background and white "T" in RGBA format
magick -size 512x512 xc:"rgba(74,144,226,1)" -pointsize 300 -fill white -font "Helvetica-Bold" -gravity center -annotate +0+0 "T" -alpha on icon.png

# Generate required sizes with RGBA
magick icon.png -resize 32x32 -alpha on 32x32.png
magick icon.png -resize 128x128 -alpha on 128x128.png
magick icon.png -resize 256x256 -alpha on 128x128@2x.png

# Create .icns for macOS
mkdir -p icon.iconset
magick icon.png -resize 16x16 icon.iconset/icon_16x16.png
magick icon.png -resize 32x32 icon.iconset/icon_16x16@2x.png
magick icon.png -resize 32x32 icon.iconset/icon_32x32.png
magick icon.png -resize 64x64 icon.iconset/icon_32x32@2x.png
magick icon.png -resize 128x128 icon.iconset/icon_128x128.png
magick icon.png -resize 256x256 icon.iconset/icon_128x128@2x.png
magick icon.png -resize 256x256 icon.iconset/icon_256x256.png
magick icon.png -resize 512x512 icon.iconset/icon_256x256@2x.png
magick icon.png -resize 512x512 icon.iconset/icon_512x512.png
magick icon.png -resize 1024x1024 icon.iconset/icon_512x512@2x.png

iconutil -c icns icon.iconset -o icon.icns
rm -rf icon.iconset

# Create .ico for Windows (placeholder)
magick icon.png -define icon:auto-resize=256,128,64,48,32,16 icon.ico

echo "✓ Icons generated successfully"
