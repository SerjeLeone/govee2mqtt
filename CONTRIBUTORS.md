# Contributors and fork lineage

This repository is maintained by [Sergey Leonov (@serjeleone)](https://github.com/serjeleone),
who contributes the fork-specific fixes, packaging, and release maintenance.

The codebase is derived from:

- [sitapix/govee2mqtt](https://github.com/sitapix/govee2mqtt), the direct source of this fork;
- [wez/govee2mqtt](https://github.com/wez/govee2mqtt), the original project by Wez Furlong.

This fork also incorporates selected fixes and features from the actively maintained sibling fork
[florianhorner/govee2mqtt-extended](https://github.com/florianhorner/govee2mqtt-extended),
including authentication resilience, LAN polling safeguards, scene catalog work, device-state
improvements, and lessons from its Home Assistant build workflow.

## Imported and adapted changes

The current integration preserves source attribution for the changes that were cherry-picked or
adapted across diverged trees:

- [sitapix `4b224c2`](https://github.com/sitapix/govee2mqtt/commit/4b224c2afecea6a864158533ea08f210c0cf0615): 2FA/app-version recovery, fan and air-quality entities, diagnostics, and device quirks;
- extended [`d5f7e35`](https://github.com/florianhorner/govee2mqtt-extended/commit/d5f7e35831927448aa38e233cf42f612e99c7d14), [`3c87648`](https://github.com/florianhorner/govee2mqtt-extended/commit/3c876486abf1645bbeb60e84f3116eea0017cfac), [`b2c7737`](https://github.com/florianhorner/govee2mqtt-extended/commit/b2c7737199fff1222d7a954f0b3e6845513f685e), and [`fbbbae4`](https://github.com/florianhorner/govee2mqtt-extended/commit/fbbbae4306b734290f99c11cb0d8939b470a438a): robust 2FA, configurable app version, unique Platform API request IDs, and bounded LAN polling;
- scene work from extended [`0666c35`](https://github.com/florianhorner/govee2mqtt-extended/commit/0666c3541afd89696c2aa2d78c550b540578da3c), [`e4542b5`](https://github.com/florianhorner/govee2mqtt-extended/commit/e4542b54b392192c3600a10c49d80e440e57a1ee), [`b84d79d`](https://github.com/florianhorner/govee2mqtt-extended/commit/b84d79d9640e2611fd3651b7e52cd7cead5582cc), [`7d993f8`](https://github.com/florianhorner/govee2mqtt-extended/commit/7d993f8bf1cd0b1ba839d0538104da2403312a8e), and [`48843cf`](https://github.com/florianhorner/govee2mqtt-extended/commit/48843cf0c7a42db470fad08deeed6abb103af634): categorized catalogs, next/previous controls, caching, animation-safe active-scene tracking, icons, and hints;
- optional extended commits [`5759ae0`](https://github.com/florianhorner/govee2mqtt-extended/commit/5759ae028f671d4cc6ea6c0327bd30d29a25cfd4), [`1962321`](https://github.com/florianhorner/govee2mqtt-extended/commit/196232160f7c2b113e5ebd89c571d84cccfc35ba), [`9e3d986`](https://github.com/florianhorner/govee2mqtt-extended/commit/9e3d98679bdf7e26e257c5ab266a9c114f507b89), [`e550412`](https://github.com/florianhorner/govee2mqtt-extended/commit/e550412430967eefc00fe19f642356b0e5d32386), and [`d4819f1`](https://github.com/florianhorner/govee2mqtt-extended/commit/d4819f1ba0021890a26e887adec1790913cfd121): IoT mode freshness, H60B0 classification, Web UI status grouping, and opt-in music palettes.

The final integration and conflict resolution are maintained by Sergey Leonov. Original authors
are also credited in the integrating commit's `Co-authored-by` trailers.

Many other people have contributed code, tests, device information, and documentation over the
project's history. The Git history and GitHub contributors pages remain the authoritative record
of individual contributions. Existing copyright notices are retained under the MIT license.
