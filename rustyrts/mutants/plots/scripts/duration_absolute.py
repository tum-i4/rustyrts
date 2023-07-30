import pandas as pd

from rustyrts.mutants.plots.scripts.labels import get_labels_mutants, url_mutants, output_format
from rustyrts.util.plotter import boxplot

y_label = "absolute e2e testing time [s]"
file = "../duration_absolute" + output_format

labels = get_labels_mutants(count=False)

df_retest_all = pd.read_sql(
    'SELECT commit as repository, retest_all_duration as y, \'retest-all\' as algorithm FROM mutant_extended', url_mutants)

df_dynamic = pd.read_sql(
    'SELECT commit as repository, dynamic_duration as y, \'dynamic\' as algorithm FROM mutant_extended', url_mutants)

df_static = pd.read_sql(
    'SELECT commit as repository, static_duration as y, \'static\' as algorithm FROM mutant_extended', url_mutants)

df = pd.concat([df_retest_all, df_dynamic, df_static])

boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])
