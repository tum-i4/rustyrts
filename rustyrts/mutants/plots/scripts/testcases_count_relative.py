import pandas as pd

from rustyrts.mutants.plots.scripts.labels import get_labels_mutants, url_mutants
from rustyrts.util.plotter import boxplot

labels = get_labels_mutants()

df_dynamic = pd.read_sql(
    'SELECT commit as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic\' as algorithm FROM testcases_count',
    url_mutants)

df_static = pd.read_sql(
    'SELECT commit as repository, static_count * 100.0 / retest_all_count as y, \'static\' as algorithm FROM testcases_count',
    url_mutants)

df = pd.concat([df_dynamic, df_static])

boxplot(df, labels, "relative number of tests [%]", "selected_tests_relative.pdf", ["#E37222", "#A2AD00"], hue='algorithm')
