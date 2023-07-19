# %%
## budget
from walkers.rustyrts_walker import walk as git_walk
from walkers.mutants_rts_walker import walk as mutants_walk

path = "../projects/small/budget"
branch = "master"
commits=[("2db4b033e5fc9ba05010def0f6988ba9b822ae8e", None, None), ("701986ccc213eae976fa8f1bd4118132a5a3f005", None, None)]

git_walk(path, branch=branch, commits=commits)
mutants_walk(path, branch=branch, commits=commits)
