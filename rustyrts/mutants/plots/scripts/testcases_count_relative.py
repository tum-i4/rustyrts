import pandas as pd

from rustyrts.mutants.plots.scripts.labels import get_labels_mutants, url_mutants, output_format
from rustyrts.util.plotter import boxplot_with_observations, boxplot, stripplot

y_label = "relative number of tests [%]"
file = "../selected_tests_relative"

labels = get_labels_mutants()

df_dynamic = pd.read_sql(
    'SELECT commit as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic\' as algorithm FROM testcases_count',
    url_mutants)

df_static = pd.read_sql(
    'SELECT commit as repository, static_count * 100.0 / retest_all_count as y, \'static\' as algorithm FROM testcases_count',
    url_mutants)

df = pd.concat([df_dynamic, df_static])

boxplot_with_observations(df, labels, y_label, file + output_format, ["#E37222", "#A2AD00"])
boxplot(df, labels, y_label, file + "_boxplot" + output_format, ["#E37222", "#A2AD00"])
stripplot(df, labels, y_label, file + "_stripplot" + output_format, ["#E37222", "#A2AD00"])
