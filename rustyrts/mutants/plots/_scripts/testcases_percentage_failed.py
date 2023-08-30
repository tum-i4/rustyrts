import pandas as pd

from rustyrts.mutants.plots._scripts.labels import get_labels_mutants, url_mutants, output_format
from rustyrts.util.plotter import boxplot_with_observations, boxplot, stripplot

y_label = "failed tests of selected tests [%]"
file = "../selected_tests_percentage_failed"

labels = get_labels_mutants(count=False)

df_retest_all = pd.read_sql(
    'SELECT commit as repository, retest_all_count_failed * 100.0 / retest_all_count as y, \'retest-all\' as algorithm FROM testcases_count'
    ' WHERE retest_all_count != 0 and dynamic_count != 0 and static_count != 0',
    url_mutants)

df_dynamic = pd.read_sql(
    'SELECT commit as repository, dynamic_count_failed * 100.0 / dynamic_count as y, \'dynamic\' as algorithm FROM testcases_count'
    ' WHERE retest_all_count != 0 and dynamic_count != 0 and static_count != 0',
    url_mutants)

df_static = pd.read_sql(
    'SELECT commit as repository, static_count_failed * 100.0 / static_count as y, \'static\' as algorithm FROM testcases_count'
    ' WHERE retest_all_count != 0 and dynamic_count != 0 and static_count != 0',
    url_mutants)

df = pd.concat([df_retest_all, df_dynamic, df_static])

boxplot(df, labels, y_label, file + output_format, ["#DAD7CB", "#E37222", "#A2AD00"], figsize=(22, 15), legend_anchor=(1.0,0.8,0.1,0.1))
