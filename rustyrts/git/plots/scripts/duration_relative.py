import pandas as pd

from rustyrts.git.plots.scripts.labels import get_labels_git, url_git, output_format
from rustyrts.util.plotter import boxplot

y_label = "relative e2e testing time [%]"
file = "../duration_relative" + output_format

labels = get_labels_git()

df_dynamic = pd.read_sql(
    'SELECT c.repo_id as repository, dynamic_duration * 100.0 / retest_all_duration as y, \'dynamic\' as algorithm '
    'FROM testreport_extended join "Commit" c on c.id = commit ORDER BY commit', url_git)

df_static = pd.read_sql(
    'SELECT c.repo_id as repository, static_duration * 100.0 / retest_all_duration as y, \'static\' as algorithm '
    'FROM testreport_extended join "Commit" c on c.id = commit ORDER BY commit', url_git)

df = pd.concat([df_dynamic, df_static])

boxplot(df, labels, y_label, file, ["#E37222", "#A2AD00"])
