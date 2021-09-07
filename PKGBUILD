# Maintainer: kpcyrd <kpcyrd[at]archlinux[dot]org>

pkgname=libredefender
pkgver=0.0.0
pkgrel=1
pkgdesc='Light-weight antivirus scanner for Linux'
url='https://github.com/kpcyrd/libredefender'
arch=('x86_64')
license=('GPL3')
depends=('clamav')
makedepends=('cargo' 'clang')
#backup=('etc/libredefender.conf')

build() {
  cd ..
  cargo build --release --locked
}

package() {
  cd ..

  install -Dm 755 -t "${pkgdir}/usr/bin" \
    target/release/libredefender

  # install completions
  install -d "${pkgdir}/usr/share/bash-completion/completions" \
             "${pkgdir}/usr/share/zsh/site-functions" \
             "${pkgdir}/usr/share/fish/vendor_completions.d"
  "${pkgdir}/usr/bin/libredefender" completions bash > "${pkgdir}/usr/share/bash-completion/completions/libredefender"
  "${pkgdir}/usr/bin/libredefender" completions zsh > "${pkgdir}/usr/share/zsh/site-functions/_libredefender"
  "${pkgdir}/usr/bin/libredefender" completions fish > "${pkgdir}/usr/share/fish/vendor_completions.d/libredefender.fish"

  install -Dm 644 contrib/libredefender.desktop -t "${pkgdir}/etc/xdg/autostart"
  install -Dm 644 contrib/icon.svg "${pkgdir}/usr/share/icons/hicolor/scalable/apps/${pkgname}.svg"
}

# vim: ts=2 sw=2 et:
