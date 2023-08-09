import pandas as pd

from rustyrts.git.plots.scripts.labels import url_git, get_labels_git, output_format
from rustyrts.util.plotter import boxplot

y_label = "absolute number of tests"
file = "../selected_targets_absolute"

labels = get_labels_git()

df_retest_all_unit = pd.read_sql(
    'SELECT c.repo_id as repository, retest_all_count as y, \'retest-all - unit\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
    url_git)

df_dynamic_unit = pd.read_sql(
    'SELECT c.repo_id as repository, dynamic_count as y, \'dynamic - unit\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
    url_git)

df_static_unit = pd.read_sql(
    'SELECT c.repo_id as repository, static_count as y, \'static - unit\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
    url_git)

df_retest_all_integration = pd.read_sql(
    'SELECT c.repo_id as repository, retest_all_count as y, \'retest-all - integration\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
    url_git)

df_dynamic_integration = pd.read_sql(
    'SELECT c.repo_id as repository, dynamic_count as y, \'dynamic - integration\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
    url_git)

df_static_integration = pd.read_sql(
    'SELECT c.repo_id as repository, static_count as y, \'static - integration\' as algorithm '
    'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
    url_git)

df_dynamic = pd.concat([df_retest_all_unit, df_retest_all_integration, df_dynamic_unit, df_dynamic_integration])
df_static = pd.concat([df_retest_all_unit, df_retest_all_integration, df_static_unit, df_static_integration])

boxplot(df_dynamic, labels, y_label, file + "_dynamic" + output_format, ["#E0DED4", "#ADABA1", "#E98C4A", "#B65C1B"])
boxplot(df_static, labels, y_label, file + "_static" + output_format, ["#E0DED4", "#ADABA1", "#B4BE26", "#818B00"])

#boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])
