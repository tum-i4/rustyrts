import pandas as pd

from rustyrts.mutants.plots._scripts.labels import get_labels_mutants, url_mutants, output_format
from rustyrts.util.plotter import boxplot, stripplot, boxplot_with_observations

y_label = "relative number of tests [%]"
file = "../selected_targets_relative"

labels = get_labels_mutants()

df_dynamic_unit = pd.read_sql(
    'SELECT  commit as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic - unit\' as algorithm '
    'FROM target_count WHERE target = \'UNIT\'',
    url_mutants)

df_static_unit = pd.read_sql(
    'SELECT  commit as repository, static_count * 100.0 / retest_all_count as y, \'static - unit\' as algorithm '
    'FROM target_count WHERE target = \'UNIT\'',
    url_mutants)

df_dynamic_integration = pd.read_sql(
    'SELECT  commit as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic - integration\' as algorithm '
    'FROM target_count WHERE target = \'INTEGRATION\'',
    url_mutants)

df_static_integration = pd.read_sql(
    'SELECT  commit as repository, static_count * 100.0 / retest_all_count as y, \'static - integration\' as algorithm '
    'FROM target_count WHERE target = \'INTEGRATION\'',
    url_mutants)

df = pd.concat([df_dynamic_unit, df_dynamic_integration, df_static_unit, df_static_integration])

boxplot_with_observations(df, labels, y_label, file + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
                          figsize=(22, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))
boxplot(df, labels, y_label, file + "_boxplot" + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
        figsize=(24, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))
stripplot(df, labels, y_label, file + "_stripplot" + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
          figsize=(24, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))
