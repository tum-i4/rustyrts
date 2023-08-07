import pandas as pd

from rustyrts.git.plots.scripts.labels import get_labels_git, url_git, output_format
from rustyrts.util.plotter import boxplot, stripplot

y_label = 'Tests that have been selected'
file = '../contains_all_tests' + output_format

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

stripplot(df, labels, y_label, file, ["#E37222"], hue='algorithm')
