# The top of README.md. The rest is each directory's `nix/readme.nix`, and
# `nix run .#write-files` puts them together.
{
  perSystem.readme.intro = ''
    # Harken

    A music, podcast, and sermon player you can use on your phone, in the browser, or desktop.

    Built with arkio.

    One server, three clients — a desktop program, a page in a browser, and a
    phone — and one `apply` between them: every mutation is written once, in
    Rust, and every peer replays the same log. Sign in once per device through
    whatever OpenID Connect provider the server is pointed at; take any peer
    offline, change things on both sides, come back, and watch the rebase.
  '';
}
