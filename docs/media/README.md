# README media

The images in the README are frames of the interactive tour,
[`source/reel.html`](source/reel.html), which is also published at
https://claude.ai/artifact/9xWiUJnmsdS2z4aYauTGik. The tour draws the app in the Workbench
design system with invented data, so the images change only when the tour changes.

To refresh them after editing `source/reel.html` (needs `pnpm install` and ImageMagick):

```
mkdir -p /tmp/shots /tmp/frames
node docs/media/source/capture.mjs shots /tmp/shots
for f in /tmp/shots/*.png; do magick "$f" -resize 1600x -strip -colors 256 PNG8:docs/media/$(basename "$f"); done
node docs/media/source/capture.mjs frames /tmp/frames
img2webp -loop 0 -lossless -m 4 -d 200 /tmp/frames/*.png -o docs/media/reel.webp
```

`icon.png` is `apps/desktop/src-tauri/icons/128x128@2x.png`.
