# Art

Every image in Planroom is made with [Iris](https://github.com/doodla/iris). Each image sits next to the exact prompt file that made it, passed with `iris image generate -f`. The look follows [Design language](../contributing/design-language.md), and `style.txt` holds the shared style notes.

| Image | Prompt | Model | Options | Estimated cost (USD) | SHA-256 |
|---|---|---|---|---|---|
| `logo.png` | `logo.prompt.txt` | `gpt-image-2.5-sunburst` | 1024x1024, high, transparent, png | 0.05338 | `1003f4ba885ed438…` |
| `hero.png` | `hero.prompt.txt` | `gpt-image-2.5-sunburst` | 1536x1024, high, png | 0.04211 | `b9ad85f83e5e7e83…` |
| `social-base.png` | `social.prompt.txt` | `gpt-image-2.5-sunburst` | 1280x640, high, png | 0.02521 | `34b48922dec6e628…` |
| `empty-map.png` | `empty-map.prompt.txt` | `gpt-image-2.5-flare` | 1024x1024, medium, transparent, png | 0.01378 | `11a3f9e6c5ef2f72…` |
| `empty-decisions.png` | `empty-decisions.prompt.txt` | `gpt-image-2.5-flare` | 1024x1024, medium, transparent, png | 0.01379 | `98479f3e1a4a0530…` |
| `empty-diffs.png` | `empty-diffs.prompt.txt` | `gpt-image-2.5-flare` | 1024x1024, medium, transparent, png | 0.01378 | `f19c20e61a3cb553…` |

Drafts at medium quality on `gpt-image-2.5-flare` came first (2 logo drafts, 1 hero draft, and a first social and empty-state set that drew interlocking studded bricks, which read as another company's toy). They aren't committed. Total Iris spend on 2026-10-25: about $0.27, against a $5 cap.

To regenerate an image, run:

```sh
iris image generate -m MODEL --size SIZE --quality QUALITY -f PROMPT_FILE -o IMAGE --max-cost 0.07
```

The social preview overlays the wordmark on `social-base.png` as text, so the letters are exact.
