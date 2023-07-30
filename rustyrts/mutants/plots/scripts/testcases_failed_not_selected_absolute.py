import pandas as pd

from rustyrts.mutants.plots.scripts.labels import url_mutants, get_labels_mutants, output_format
from rustyrts.util.plotter import boxplot

y_label = 'Failed tests, not selected'
file = '../failed_but_not_selected_absolute' + output_format

labels = get_labels_mutants()


def get_test_diff(retest_all, other):
    retest_all_tests = retest_all.splitlines()
    other_tests = other.splitlines()
    return list(set(retest_all_tests) - set(other_tests))


df_failed_retest_all = pd.read_sql(
    'SELECT commit as repository, retest_all_mutant_id, retest_all_failed , descr as mutant FROM testcases_failed WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

df_selected_dynamic = pd.read_sql(
    'SELECT commit as repository, retest_all_mutant_id, dynamic, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

df_selected_static = pd.read_sql(
    'SELECT commit as repository, retest_all_mutant_id, static, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
    url_mutants)

should_be_selected = []
not_selected_dynamic = []
not_selected_static = []

failed_retest_all = df_failed_retest_all.to_dict(orient='records')
selected_dynamic = df_selected_dynamic.to_dict(orient='records')
selected_static = df_selected_static.to_dict(orient='records')

assert len(failed_retest_all) == len(selected_dynamic) and len(failed_retest_all) == len(selected_static)

for (retest_all_mutant, dynamic_mutant, static_mutant) in zip(failed_retest_all, selected_dynamic, selected_static):
    assert retest_all_mutant['retest_all_mutant_id'] == dynamic_mutant['retest_all_mutant_id']
    assert retest_all_mutant['retest_all_mutant_id'] == static_mutant['retest_all_mutant_id']

    repository = retest_all_mutant['repository']
    descr = retest_all_mutant['mutant']

    diff_dynamic = get_test_diff(retest_all_mutant['retest_all_failed'], dynamic_mutant['dynamic'])
    diff_static = get_test_diff(retest_all_mutant['retest_all_failed'], static_mutant['static'])

    should_be_selected.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'retest-all',
         'y': len(retest_all_mutant['retest_all_failed'].splitlines())})
    not_selected_dynamic.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'dynamic', 'y': len(diff_dynamic)})
    not_selected_static.append(
        {'repository': repository, 'mutant': descr, 'algorithm': 'static', 'y': len(diff_static)})

df_failed_retest_all = pd.DataFrame(should_be_selected)
df_not_selected_dynamic = pd.DataFrame(not_selected_dynamic)
df_not_selected_static = pd.DataFrame(not_selected_static)

df = pd.concat([df_failed_retest_all[['repository', 'algorithm', 'y']],
                df_not_selected_dynamic[['repository', 'algorithm', 'y']],
                df_not_selected_static[['repository', 'algorithm', 'y']]])

boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"], hue='algorithm')
