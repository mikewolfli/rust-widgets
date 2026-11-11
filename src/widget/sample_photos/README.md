# Sample photographs

The four JPEGs in this directory are the images `widget::sample_fill` installs into the
`image_gallery` sample, so the control's snapshot shows a decoded photograph instead of a
placeholder well.

They are **generated scenes, not camera photographs**: this repository ships no photographic
assets and the build environment has no network access. `tools/make_sample_photos.py` is the
generator; it uses fixed seeds, so re-running it writes byte-identical files.

## Replacing them with real photographs

Drop four files with the same names into this directory:

```
coast.jpg  forest.jpg  city.jpg  desert.jpg
```

Any JPEG, PNG or WebP that `image::Image::from_bytes` decodes works — the sample reads the
bytes, so no code change is needed. Keep them small (a few hundred pixels per side); they are
embedded in the library's sample-data module and therefore in the test/demo binary.

The source of a photograph must be recorded here (author, licence and URL) before it is
committed, because these files ship under this repository's MIT licence.
