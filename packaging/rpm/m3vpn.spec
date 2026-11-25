Name:           m3vpn
Version:        @VERSION@
Release:        1%{?dist}
Summary:        Material 3 VPN client for Linux
License:        MIT
URL:            https://github.com/nikitaakbaev/m3vpn
Requires:       gtk3
Requires:       webkit2gtk4.1
AutoReqProv:    no
%global debug_package %{nil}

%description
m3vpn is a Material 3 client for Xray, sing-box, Reality, Hysteria 2,
AmneziaWG and TUN mode.

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a %{stage}/. %{buildroot}/

%files
/usr/bin/m3vpn
/usr/lib/m3vpn
/usr/lib/systemd/system/m3vpn-helper.service
/usr/share/applications/cc.m3vpn.desktop
/usr/share/metainfo/cc.m3vpn.desktop.metainfo.xml
/usr/share/icons/hicolor
/usr/share/doc/m3vpn
/usr/share/licenses/m3vpn

%post
if ! getent group m3vpn >/dev/null; then
  groupadd --system m3vpn || true
fi
systemctl daemon-reload >/dev/null 2>&1 || true
systemctl enable --now m3vpn-helper.service >/dev/null 2>&1 || true
if [ -n "${SUDO_USER:-}" ] && id "$SUDO_USER" >/dev/null 2>&1; then
  usermod -aG m3vpn "$SUDO_USER" || true
fi

%preun
systemctl disable --now m3vpn-helper.service >/dev/null 2>&1 || true

%postun
systemctl daemon-reload >/dev/null 2>&1 || true

%changelog
* Wed Nov 25 2026 m3vpn <m3vpn@localhost> - 0.2.4-1
- Split tunneling, DNS policy, auto-update and packaging polish
