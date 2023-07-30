import pandas as pd

from rustyrts.mutants.plots.scripts.labels import get_labels_mutants, url_mutants, output_format
from rustyrts.util.plotter import boxplot

y_label = "relative e2e testing time [%]"
file = "../duration_relative" + output_format

labels = get_labels_mutants()

df_dynamic = pd.read_sql(
    'SELECT commit as repository, dynamic_duration * 100.0 / retest_all_duration as y, \'dynamic\' as algorithm FROM mutant_extended', url_mutants)

df_static = pd.read_sql(
    'SELECT commit as repository, static_duration * 100.0 / retest_all_duration as y, \'static\' as algorithm FROM mutant_extended', url_mutants)

df = pd.concat([df_dynamic, df_static])

boxplot(df, labels, y_label, file, ["#E37222", "#A2AD00"])
