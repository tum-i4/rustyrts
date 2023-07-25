import pandas as pd

from rustyrts.git.plots.scripts.labels import url_git, get_labels_git
from rustyrts.util.plotter import boxplot

y_label = "absolute e2e testing time [s]"
file = "../duration_absolute.pdf"

labels = get_labels_git()

df_retest_all = pd.read_sql(
    'SELECT c.repo_id as repository, retest_all_duration as y, \'retest-all\' as algorithm FROM testreport_extended join "Commit" c '
    'on c.id = commit', url_git)

df_dynamic = pd.read_sql(
    'SELECT c.repo_id as repository, dynamic_duration as y, \'dynamic\' as algorithm FROM testreport_extended join "Commit" c '
    'on c.id = commit', url_git)

df_static = pd.read_sql(
    'SELECT c.repo_id as repository, static_duration as y, \'static\' as algorithm FROM testreport_extended join "Commit" c '
    'on c.id = commit', url_git)

df = pd.concat([df_retest_all, df_dynamic, df_static])

boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])
