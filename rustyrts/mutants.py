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

path = "../projects/mutants/rust-openssl"
path = os.path.abspath(path)
branch = "master"
commits = ["e96addaa919e1f91c9dc143a9b13b218835f2356", "d85e2a293778d2a01d715060e7516a8828b0a5ac",
           "a0b56c437803a08413755928040a0970a93a7b83"]

walk(path, branch=branch, commits=commits)

# %%
## rustls
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/rustls"
path = os.path.abspath(path)
branch = "main"
commits = ["45197b807cf0699c842fcb85eb8eca555c74cc04", "bc754a4fbb586beb7b1dfce38ab880fd90c0e422",
           "24a5c11d666ddb05976877034a048e2dcaa8b80d"]

walk(path, branch=branch, commits=commits)

# %%
## tabled
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/tabled"
path = os.path.abspath(path)
branch = "master"
commits = ["cd2253e4b455431fa46e9776bd89297afc9988b8", "da08635a51bb3d5b7c42d676cb0d0fabc0af124e",
           "cc4a110d5963b7eede0e634c83c44d9e8b8250e4"]

walk(path, branch=branch, commits=commits)

# %%
## tonic
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/tonic"
path = os.path.abspath(path)
branch = "master"
commits = ["3a497f2ca2c2152cc0cfffc0c18365cd2aa3afa9", "ec359ba35fe91cd3d6b8ad2596127cc26aadcef5",
           "e87db5280e93ae4ebd14ec272cd17b6dbf9699fb"]

walk(path, branch=branch, commits=commits)

# %%
## tracing
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/tracing"
path = os.path.abspath(path)
branch = "master"
commits = ["4f1e46306d4d364fcc69691fdb29a676c7105f72", "df4ba17d857db8ba1b553f7b293ac8ba967a42f8",
           "748a1bf06efe58eaaccdcd915d7337f0b775f827"]

walk(path, branch=branch, commits=commits)

# %%
## trust-dns
import os
from walkers.mutants_rts_walker import walk

path = "../projects/mutants/trust-dns"
path = os.path.abspath(path)
branch = "main"
commits = ["fc58a4fe20da679727a3d9137a7ce833faa60dd0", "931ff130b87c4f946d36a1ac55eb9fe15e54c57c",
           "66eb9daa7d9a659f18c370c1c9fcff4e0997f9ad"]

walk(path, branch=branch, commits=commits)
