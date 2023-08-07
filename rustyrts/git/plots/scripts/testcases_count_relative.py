import pandas as pd

from rustyrts.git.plots.scripts.labels import url_git, get_labels_git, output_format
from rustyrts.util.plotter import boxplot_with_observations

labels = get_labels_git()
file = "../selected_tests_relative" + output_format

df_dynamic = pd.read_sql(
    'SELECT c.repo_id as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic\' as algorithm '
    'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
    url_git)

df_static = pd.read_sql(
    'SELECT c.repo_id as repository, static_count * 100.0 / retest_all_count as y, \'static\' as algorithm '
    'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
    url_git)

df = pd.concat([df_dynamic, df_static])

boxplot_with_observations(df, labels, "relative number of tests [%]", file, ["#E37222", "#A2AD00"], hue='algorithm')
