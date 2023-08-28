import pandas as pd

from rustyrts.git.plots._scripts.labels import get_labels_git, url_git, output_format
from rustyrts.util.plotter import boxplot, stripplot

y_label = 'Tests that have been selected'
file = '../contains_all_tests'

labels = get_labels_git()


def get_test_diff(retest_all, other):
    retest_all_tests = retest_all.splitlines()
    other_tests = other.splitlines()
    return list(set(retest_all_tests) - set(other_tests))


df_selected_dynamic = pd.read_sql(
    'SELECT c.repo_id as repository, commit, retest_all, dynamic FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

df_selected_static = pd.read_sql(
    'SELECT c.repo_id as repository, commit, retest_all, static FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
    url_git)

not_selected_static = []

selected_dynamic = df_selected_dynamic.to_dict(orient='records')
selected_static = df_selected_static.to_dict(orient='records')

assert len(selected_static) == len(selected_static)

for (dynamic_report, static_report) in zip(selected_dynamic, selected_static):
    assert dynamic_report['commit'] == static_report['commit']

    repository = static_report['repository']
    commit = static_report['commit']

    diff = get_test_diff(dynamic_report['dynamic'], static_report['static'])

    not_selected_static.append(
        {'repository': repository, 'commit': commit, 'algorithm': 'dynamic but not static', 'y': len(diff)})

df_not_selected_static = pd.DataFrame(not_selected_static)
df = pd.concat([df_not_selected_static[['repository', 'algorithm', 'y']]])

filter_normal = [1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12]
filter_special = [8]

labels1 = labels[:7] + labels[8:]
labels2 = [labels[7]]

df_1 = df[(df["repository"].isin(filter_normal))]
df_2 = df[(df["repository"].isin(filter_special))]



#stripplot(df, labels, y_label, file, ["#E37222"], hue='algorithm')
stripplot(df_1, labels1, y_label, file + "_1" + output_format,
          ["#E37222"], hue='algorithm', figsize=(18, 15), legend_anchor=(0.3,0.9,0.7,0.1), single_threaded=("single" in url_git))
stripplot(df_2, labels2, "", file + "_2" + output_format,["#E37222"],
          hue='algorithm', figsize=(3, 15),
          legend=False)