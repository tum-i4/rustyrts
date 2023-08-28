import pandas as pd

from rustyrts.git.plots._scripts.labels import url_git, get_labels_git, output_format
from rustyrts.util.plotter import boxplot

y_label = "absolute number of tests"
file = "../selected_tests_absolute" + output_format

labels = get_labels_git()

df_retest_all = pd.read_sql(
    'SELECT c.repo_id as repository, retest_all_count as y, \'retest-all\' as algorithm '
    'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
    url_git)

df_dynamic = pd.read_sql(
    'SELECT c.repo_id as repository, dynamic_count as y, \'dynamic\' as algorithm '
    'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
    url_git)

df_static = pd.read_sql(
    'SELECT c.repo_id as repository, static_count as y, \'static\' as algorithm '
    'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
    url_git)

df = pd.concat([df_retest_all, df_dynamic, df_static])

boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"], single_threaded=("single" in url_git))
