import pandas as pd

from rustyrts.git.plots.scripts.labels import url_git, get_labels_git, output_format
from rustyrts.util.plotter import boxplot, stripplot, boxplot_with_observations

y_label = "relative number of tests [%]"
file = "../selected_targets_relative"

labels = get_labels_git()

df_dynamic_unit = pd.read_sql(
    'SELECT c.repo_id as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic - unit\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
    url_git)

df_static_unit = pd.read_sql(
    'SELECT c.repo_id as repository, static_count * 100.0 / retest_all_count as y, \'static - unit\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
    url_git)

df_dynamic_integration = pd.read_sql(
    'SELECT c.repo_id as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic - integration\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
    url_git)

df_static_integration = pd.read_sql(
    'SELECT c.repo_id as repository, static_count * 100.0 / retest_all_count as y, \'static - integration\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
    url_git)

df = pd.concat([df_dynamic_unit, df_dynamic_integration, df_static_unit, df_static_integration])

boxplot_with_observations(df, labels, y_label, file + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"])
boxplot(df, labels, y_label, file + "_boxplot" + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"])
stripplot(df, labels, y_label, file + "_stripplot" + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"])
