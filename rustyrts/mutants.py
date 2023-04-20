# %%
## orion
from rustyrts.walkers.mutants_rts_walker import walk

path = "/home/simon/Dokumente/Semester10/GuidedResearch/Projects/Suitable/Short/orion"
branch = "master"

walk(path, branch=branch, commits=["da08f78b003b1450f5a9e94faaba6318fc9fc274"])
