import pandas as pd

from rustyrts.git.plots._scripts.labels import url_git, get_labels_git, output_format
from rustyrts.util.plotter import boxplot_with_observations, stripplot, boxplot

y_label = "relative number of tests [%]"
file = "../selected_tests_relative"

labels = get_labels_git()

df_dynamic = pd.read_sql(
    'SELECT c.repo_id as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic\' as algorithm '
    'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
    url_git)

df_static = pd.read_sql(
    'SELECT c.repo_id as repository, static_count * 100.0 / retest_all_count as y, \'static\' as algorithm '
    'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
    url_git)

df = pd.concat([df_dynamic, df_static])

boxplot_with_observations(df, labels, y_label, file + output_format, ["#E37222", "#A2AD00"],
                          single_threaded=("single" in url_git))
boxplot(df, labels, y_label, file + "_boxplot" + output_format, ["#E37222", "#A2AD00"],
        single_threaded=("single" in url_git))
stripplot(df, labels, y_label, file + "_stripplot" + output_format, ["#E37222", "#A2AD00"],
          single_threaded=("single" in url_git))
