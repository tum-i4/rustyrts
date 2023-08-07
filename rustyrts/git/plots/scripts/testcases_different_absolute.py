import pandas as pd

from rustyrts.git.plots.scripts.labels import url_git, get_labels_git, output_format
from rustyrts.util.plotter import stripplot

y_label_selected = 'Tests with different result, selected'
file_selected = '../different_and_selected_absolute' + output_format

y_label_not_selected = 'Tests with different result, not selected'
file_not_selected = '../different_and_not_selected_absolute' + output_format

labels = get_labels_git()


def get_test_diff_and_intersection(retest_all, other):
    retest_all_tests = set(retest_all.splitlines())
    other_tests = set(other.splitlines())
    return list(retest_all_tests.difference(other_tests)), list(retest_all_tests.intersection(other_tests))


df_different_retest_all = pd.read_sql(
    'SELECT c.repo_id as repository, retest_all_different , commit as commit FROM testcases_newly_different join "Commit" c '
    'on c.id = commit ORDER BY commit',
    url_git)

df_selected_dynamic = pd.read_sql(
    'SELECT  c.repo_id as repository, dynamic, commit as commit FROM testcases_selected join "Commit" c on c.id = '
    'commit ORDER BY commit',
    url_git)

df_selected_static = pd.read_sql(
    'SELECT  c.repo_id as repository, static, commit as commit FROM testcases_selected join "Commit" c on c.id = '
    'commit ORDER BY commit',
    url_git)

selected_dynamic = []
not_selected_dynamic = []
selected_static = []
not_selected_static = []

different_retest_all_count = {}

raw_different_retest_all = df_different_retest_all.to_dict(orient='records')
raw_selected_dynamic = df_selected_dynamic.to_dict(orient='records')
raw_selected_static = df_selected_static.to_dict(orient='records')

assert len(raw_different_retest_all) == len(raw_selected_dynamic) and len(raw_different_retest_all) == len(
    raw_selected_static)

for (retest_all_report, dynamic_report, static_report) in zip(raw_different_retest_all, raw_selected_dynamic,
                                                              raw_selected_static):
    repository = retest_all_report['repository']
    commit = retest_all_report['commit']

    if repository not in different_retest_all_count:
        different_retest_all_count[repository] = {}
        different_retest_all_count[repository]["count"] = 0
        different_retest_all_count[repository]["commits"] = 0
    count = len(set(retest_all_report['retest_all_different'].splitlines()))
    if count > 0:
        different_retest_all_count[repository]["count"] += count
        different_retest_all_count[repository]["commits"] += 1

    (diff_dynamic, intersection_dynamic) = get_test_diff_and_intersection(retest_all_report['retest_all_different'],
                                                                          dynamic_report['dynamic'])
    (diff_static, intersection_static) = get_test_diff_and_intersection(retest_all_report['retest_all_different'],
                                                                        static_report['static'])

    selected_dynamic.append(
        {'repository': repository, 'commit': commit, 'algorithm': 'dynamic',
         'y': len(intersection_dynamic)})
    not_selected_dynamic.append(
        {'repository': repository, 'commit': commit, 'algorithm': 'dynamic',
         'y': len(diff_dynamic)})
    selected_static.append(
        {'repository': repository, 'commit': commit, 'algorithm': 'static',
         'y': len(intersection_static)})
    not_selected_static.append(
        {'repository': repository, 'commit': commit, 'algorithm': 'static',
         'y': len(diff_static)})

df_selected_dynamic = pd.DataFrame(selected_dynamic)
df_selected_static = pd.DataFrame(selected_static)

df_not_selected_dynamic = pd.DataFrame(not_selected_dynamic)
df_not_selected_static = pd.DataFrame(not_selected_static)

for i in range(len(labels)):
    labels[i] += "\n(" + str(different_retest_all_count[i + 1]["count"]) + " on " + str(
        different_retest_all_count[i + 1]["commits"]) + ")"

df_selected = pd.concat([df_selected_dynamic[['repository', 'algorithm', 'y']],
                         df_selected_static[['repository', 'algorithm', 'y']]])

df_not_selected = pd.concat([df_not_selected_dynamic[['repository', 'algorithm', 'y']],
                             df_not_selected_static[['repository', 'algorithm', 'y']]])

stripplot(df_selected, labels, y_label_selected, file_selected, ["#E37222", "#A2AD00"], hue='algorithm')
stripplot(df_not_selected, labels, y_label_not_selected, file_not_selected, ["#E37222", "#A2AD00"], hue='algorithm')
