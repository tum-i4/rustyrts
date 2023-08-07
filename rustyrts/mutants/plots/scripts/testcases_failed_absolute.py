import pandas as pd

from rustyrts.mutants.plots.scripts.labels import url_mutants, get_labels_mutants, output_format
from rustyrts.util.plotter import stripplot

y_label_selected = 'Failed tests, selected'
file_selected = '../failed_and_selected_absolute'

y_label_not_selected = 'Failed tests, not selected'
file_not_selected = '../failed_and_not_selected_absolute'

labels = get_labels_mutants()


def get_test_diff_and_intersection(retest_all, other):
    retest_all_tests = set(retest_all.splitlines())
    other_tests = set(other.splitlines())
    return list(retest_all_tests.difference(other_tests)), list(retest_all_tests.intersection(other_tests))


df_failed_retest_all = pd.read_sql(
    'SELECT commit as repository, retest_all_mutant_id, retest_all_failed , descr as mutant FROM testcases_failed WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

df_selected_dynamic = pd.read_sql(
    'SELECT commit as repository, retest_all_mutant_id, dynamic, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

df_selected_static = pd.read_sql(
    'SELECT commit as repository, retest_all_mutant_id, static, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

selected_dynamic = []
not_selected_dynamic = []
selected_static = []
not_selected_static = []

raw_failed_retest_all = df_failed_retest_all.to_dict(orient='records')
raw_selected_dynamic = df_selected_dynamic.to_dict(orient='records')
raw_selected_static = df_selected_static.to_dict(orient='records')

assert len(raw_failed_retest_all) == len(raw_selected_dynamic) and len(raw_failed_retest_all) == len(
    raw_selected_static)

for (retest_all_mutant, dynamic_mutant, static_mutant) in zip(raw_failed_retest_all, raw_selected_dynamic,
                                                              raw_selected_static):
    assert retest_all_mutant['retest_all_mutant_id'] == dynamic_mutant['retest_all_mutant_id']
    assert retest_all_mutant['retest_all_mutant_id'] == static_mutant['retest_all_mutant_id']

    repository = retest_all_mutant['repository']
    descr = retest_all_mutant['mutant']

    (diff_dynamic, intersection_dynamic) = get_test_diff_and_intersection(retest_all_mutant['retest_all_failed'],
                                                                          dynamic_mutant['dynamic'])
    (diff_static, intersection_static) = get_test_diff_and_intersection(retest_all_mutant['retest_all_failed'],
                                                                        static_mutant['static'])

    selected_dynamic.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'dynamic', 'y': len(intersection_dynamic)})
    not_selected_dynamic.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'dynamic', 'y': len(diff_dynamic)})
    selected_static.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'static', 'y': len(intersection_static)})
    not_selected_static.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'static', 'y': len(diff_static)})

df_selected_dynamic = pd.DataFrame(selected_dynamic)
df_not_selected_dynamic = pd.DataFrame(not_selected_dynamic)
df_selected_static = pd.DataFrame(selected_static)
df_not_selected_static = pd.DataFrame(not_selected_static)

df_selected = pd.concat([
    df_selected_dynamic[['repository', 'algorithm', 'y']],
    df_selected_static[['repository', 'algorithm', 'y']]
])
df_not_selected = pd.concat([
    df_not_selected_dynamic[['repository', 'algorithm', 'y']],
    df_not_selected_static[['repository', 'algorithm', 'y']]
])

filter_normal = [1, 2, 3, 5, 6, 7, 8, 9, 10]
filter_special = [4]

labels1 = labels[:3] + labels[4:]
labels2 = [labels[3]]

# df_selected_1 = df_selected[(df_selected["repository"].isin(filter_normal))]
# df_selected_2 = df_selected[(df_selected["repository"].isin(filter_special))]
df_not_selected_1 = df_not_selected[(df_not_selected["repository"].isin(filter_normal))]
df_not_selected_2 = df_not_selected[(df_not_selected["repository"].isin(filter_special))]

# stripplot(df_selected_1, labels1, y_label_selected, file_selected + "_1" + output_format, ["#E37222", "#A2AD00"],
#          hue='algorithm', figsize=(17, 15))
# stripplot(df_selected_2, labels2, "", file_selected + "_2" + output_format, ["#E37222", "#A2AD00"], hue='algorithm',
#          figsize=(3, 15),
#          legend=False)
stripplot(df_selected, labels, y_label_selected, file_selected + output_format, ["#E37222", "#A2AD00"],
          hue='algorithm', figsize=(17, 15))

stripplot(df_not_selected_1, labels1, y_label_not_selected, file_not_selected + "_1" + output_format,
          ["#E37222", "#A2AD00"], hue='algorithm', figsize=(18, 15), legend_anchor=(0.1,0.9,0.2,0.1))
stripplot(df_not_selected_2, labels2, "", file_not_selected + "_2" + output_format, ["#E37222", "#A2AD00"],
          hue='algorithm', figsize=(3, 15),
          legend=False)
# stripplot(df_not_selected, labels, y_label_not_selected, file_not_selected + output_format,
#          ["#E37222", "#A2AD00"], hue='algorithm', figsize=(17, 15))
