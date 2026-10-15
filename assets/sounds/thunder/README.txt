Recorded thunder, compiled into the executable (crates/omsi-app/build.rs).

Put .wav files here (any sample rate, mono or stereo, 8/16/24/32-bit or 32-bit float) and build:
  near*.wav / close*.wav    a strike closer than 4 km (the crack, right overhead)
  mid*.wav                  4 to 12 km (the roll)
  far*.wav / distant*.wav   farther than 12 km (the rumble)
  anything else             used for every distance
One of the right distance is picked at random for each strike. Without any file here the game
makes its thunder up (omsi-audio/src/thunder.rs). A Sounds\Thunder\ folder in the content root,
if it holds .wav files, is used instead of these.
