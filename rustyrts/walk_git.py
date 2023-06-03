# %%
## actix/actix-web
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/actix-web"
branch = "master"

walk(path, branch=branch)


# %%
## arrow-datafusion
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/arrow-datafusion"
branch = "main"

walk(path, branch=branch)


# %%
## feroxbuster
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/feroxbuster"
branch = "main"

walk(path, branch=branch)


# %%
## nushell
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/nushell"
branch = "main"

walk(path, branch=branch)


# %%
## ockam
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/ockam"
branch = "develop"

walk(path, branch=branch)


# %%
## rayon
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/rayon"
branch = "master"

walk(path, branch=branch)


# %%
## rust-analyzer
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/rust-analyzer"
branch = "master"

walk(path, branch=branch)


# %%
## wasmer
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/wasmer"
branch = "master"

options = ["--features", "test-singlepass,test-cranelift,test-universal"]

walk(path, branch=branch, build_options=options)


# %%
## zenoh
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/zenoh"
branch = "master"

walk(path, branch=branch)


# %%
## exonum
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/exonum"
branch = "master"

walk(path, branch=branch)


# %%
## tantivy
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/tantivy"
branch = "main"

walk(path, branch=branch)


# %%
## meilisearch
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/meilisearch"
branch = "main"

walk(path, branch=branch)


# %%
## sled
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/sled"
branch = "main"

options = ["--features", "testing"]

walk(path, branch=branch, build_options=options)


# %%
## penumbra
from rustyrts.walkers.rustyrts_walker import walk

path = "../projects/git_walk/penumbra"
branch = "main"

walk(path, branch=branch)