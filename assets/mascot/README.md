# Tafo ferret

The approved ferret was expanded into three hand-drawn eight-frame sprite atlases
using the built-in image generation tool, followed by background cleanup using
that same tool. Generation prompts are in `prompts.json`.

Each atlas has four columns and two rows, read left-to-right, top-to-bottom.
AppKit displays the actual drawn frames. Idle holds its resting frame before a
blink and tail curl; switch and correction play once and hold their final pose.
Native rendering respects macOS Reduce Motion with static representative frames.
The app does not move or scale a still pose to simulate character animation.
