# %%
## orion
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/orion"
path = os.path.abspath(path)
branch = "master"
commits = ["cfa2c0c1e89f1ec3d2ab1ab1d57f88c1201e452c", "69fc37ab9c3ecf1b07020dcc2b64ea76a44500d6",
           "877a4296f3111c246cf92b5e96c25ab27696a108"]

walk(path, branch=branch, commits=commits)

# %%
## pulldown-cmark
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/pulldown-cmark"
path = os.path.abspath(path)
branch = "master"
commits = ["967dd38554399573279855a9e124dc598a0e3200", "d4bf0872b14f68c1afedee918710fb401e3e6e9a",
           "ab7774ab086ceaf1d846893550efd3c96eed5319"]

walk(path, branch=branch, commits=commits)

# %%
## regex
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/regex"
path = os.path.abspath(path)
branch = "master"
commits = ["b5ef0ec281220d9047fed199ed48c29af9749570", "cbfc0a38dee98f77b796a3712214e4c9b43162af",
           "4f2bdcbc7b30aa5122bdd416950a0494922851c5"]

walk(path, branch=branch, commits=commits)

# %%
## ripgrep
import os
from walkers.mutants_rts_walker import walk
from pathlib import Path

path = "../projects/mutants/ripgrep"
path = os.path.abspath(path)
branch = "master"
commits = ["af6b6c543b224d348a8876f0c06245d9ea7929c5", "b9cd95faf18ed6914b3b20720bb9e5ea4cffa5b9",
           "d7f57d9aabf90967c9b4374f2e22485b57993f00"]


# On the second and third commit of ripgrep, the baseline fails due to \0 being printed instead of \u{0}
# The reason for this may be some difference in stdlib, but can be fixed by replacing these tokens in the source code
def replace_u0():
    project_dir = Path(path)
    for file in project_dir.rglob("*.rs"):
        file.write_text(file.read_text().replace('u{0}', '0'))


walk(path, branch=branch, commits=commits, pre_hook=replace_u0)

# %%
## rust-brotli
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/rust-brotli"
path = os.path.abspath(path)
branch = "master"
commits = ["b1f5aed58287cb01795a099230faa7d2ac734740", "db474a19af31b2e9756981dc82c49ef9afbfc494",
           "73b6b98eaf1d61a89dd92d9ad21e84ca771290c6"]

walk(path, branch=branch, commits=commits)

# %%
## rust-openssl
import os
from walkers.mutants_rts_walker import walk
from pathlib import Path

path = "../projects/mutants/rust-openssl"
path = os.path.abspath(path)
branch = "master"
commits = ["e96addaa919e1f91c9dc143a9b13b218835f2356", "e76289f6addb9e5e11f640c590ae13a0b87dc557",
           "c2f6dcb6e3969fcc767290be6be925aa0ef1cb9a"]


# Using dynamic rts, this test just keeps failing on the remote machine, while succeeding locally
# Apparently, the reason has something to do with the operating system and not RustyRTSS itself
# This is why, we just ignore these tests
def replace_problematic_tests():
    project_dir = Path(path)

    problematic_tests_indented = ["from_nid"]
    problematic_tests = ["basic", "dynamic_data", "static_data"]

    for file in project_dir.rglob("*.rs"):
        input = file.read_text()
        for test in problematic_tests:
            sig = "#[test]\nfn " + test + "() {"
            input = input.replace(sig, "#[ignore]\n" + sig)
        for test in problematic_tests_indented:
            sig = "    #[test]\n    fn " + test + "() {"
            input = input.replace(sig, "    #[ignore]\n" + sig)
        file.write_text(input)


walk(path, branch=branch, commits=commits, pre_hook=replace_problematic_tests)

# %%
## rustls
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/rustls"
path = os.path.abspath(path)
branch = "main"
commits = ["45197b807cf0699c842fcb85eb8eca555c74cc04", "9b5bb50d9df22a2d2155a7bf35155a24824c40a6",
           "a863fc554aca02533a422dc228bcc938d20a721f"]

walk(path, branch=branch, commits=commits)

# %%
## tabled
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/tabled"
path = os.path.abspath(path)
branch = "master"
commits = ["cc4a110d5963b7eede0e634c83c44d9e8b8250e4", "d055382d4865622d94800cfd5fd1ef2784e1b14b",
           "1dadeff6eca6f1ba80415de4ffdd6728117da663"]

walk(path, branch=branch, commits=commits)

# %%
## tracing
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/tracing"
path = os.path.abspath(path)
branch = "master"
commits = ["4f1e46306d4d364fcc69691fdb29a676c7105f72", "df4ba17d857db8ba1b553f7b293ac8ba967a42f8",
           "d9e8eceafda5d4f8c36f556bd9c468edb79b8dd3"]

walk(path, branch=branch, commits=commits)