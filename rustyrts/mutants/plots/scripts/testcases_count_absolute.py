import pandas as pd

from rustyrts.mutants.plots.scripts.labels import get_labels_mutants, url_mutants, output_format
from rustyrts.util.plotter import boxplot

y_label = "absolute number of tests"
file = "../selected_tests_absolute" + output_format

labels = get_labels_mutants()

df_retest_all = pd.read_sql(
    'SELECT commit as repository, retest_all_count as y, \'retest-all\' as algorithm FROM testcases_count', url_mutants)

df_dynamic = pd.read_sql(
    'SELECT commit as repository, dynamic_count as y, \'dynamic\' as algorithm FROM testcases_count', url_mutants)

df_static = pd.read_sql(
    'SELECT commit as repository, static_count as y, \'static\' as algorithm FROM testcases_count', url_mutants)

df = pd.concat([df_retest_all, df_dynamic, df_static])

boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])
