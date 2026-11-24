# Third-party components bundled by the Linux packages

The m3vpn package ships the following upstream proxy cores. They are separate
works with their own licenses; the m3vpn source tree does not relicense them.

| Component | Source | Version / commit |
|---|---|---|
| Xray-core | https://github.com/XTLS/Xray-core | see `sidecars/versions.json` |
| hiddify-sing-box | https://github.com/hiddify/hiddify-sing-box | see `sidecars/versions.json` |
| GeoIP / GeoSite data | https://github.com/Loyalsoldier/v2ray-rules-dat | see `sidecars/versions.json` |

The license text of hiddify-sing-box is installed at
`/usr/share/licenses/m3vpn/LICENSE.sing-box`. Check the upstream repositories
for the Xray-core and geo-data licenses before redistributing the binaries.
