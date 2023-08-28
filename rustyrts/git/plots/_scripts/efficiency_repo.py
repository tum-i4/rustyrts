import pandas as pd

from rustyrts.git.plots._scripts.labels import get_labels_git, url_git, output_format
from rustyrts.util.plotter import boxplot, scatterplot

reg_label = "linear regression x~log(y)"
y_label = "average relative e2e testing time [%]"
x_label = "average absolute e2e testing time of retest-all [s]"
file = "../efficiency_repo" + output_format

df_dynamic = pd.read_sql(
    'SELECT retest_all_mean as x, dynamic_mean_relative * 100.0 as y, \'dynamic\' as algorithm FROM duration', url_git)

df_static = pd.read_sql(
    'SELECT retest_all_mean as x, static_mean_relative * 100.0 as y, \'static\' as algorithm FROM duration', url_git)

scatterplot([df_dynamic, df_static], ["dynamic - " + reg_label, "static - " + reg_label], x_label, y_label, file,
            ["#E37222", "#A2AD00"], single_threaded=("single" in url_git))
