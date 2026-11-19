# The image of podvm (the owner's ruling "ghcr.io", 2026-11-19; call C789
# of podcrates; TODO/cli.md T-1992): the static binary of a release on an
# empty base. .github/workflows/image.yml builds it for each architecture
# from the files of the release, and pushes ghcr.io/podutils/podvm:<tag>.
#
# The context has the binary `podvm` only. There is no shell: the image
# carries the binary, and `docker run IMAGE --version` runs it.
FROM scratch
COPY --chmod=0755 podvm /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/podvm"]
CMD ["--help"]
