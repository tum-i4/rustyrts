import pandas as pd
import seaborn as sns
import matplotlib as mpl
import matplotlib.pyplot as plt

from rustyrts_eval.cli.analyze.labels import get_labels_git, get_labels_mutants

output_format = ".svg"


def get_test_diff_and_intersection(retest_all, other):
    retest_all_tests = set(retest_all.splitlines())
    other_tests = set(other.splitlines())
    return list(retest_all_tests.difference(other_tests)), list(retest_all_tests.intersection(other_tests))


########################################################################################################################
# History plots

def plot_history_duration_absolute(url, sequential):
    labels = get_labels_git(url)

    y_label = "absolute e2e testing time [s]"
    file = "../duration_absolute" + output_format

    df_retest_all = pd.read_sql(
        'SELECT c.repo_id as repository, retest_all_duration as y, \'retest-all\' as algorithm FROM testreport_extended join "Commit" c '
        'on c.id = commit ORDER BY commit', url)

    df_dynamic = pd.read_sql(
        'SELECT c.repo_id as repository, dynamic_duration as y, \'dynamic\' as algorithm FROM testreport_extended join "Commit" c '
        'on c.id = commit ORDER BY commit', url)

    df_static = pd.read_sql(
        'SELECT c.repo_id as repository, static_duration as y, \'static\' as algorithm FROM testreport_extended join "Commit" c '
        'on c.id = commit ORDER BY commit', url)

    df = pd.concat([df_retest_all, df_dynamic, df_static])

    boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"], sequential=sequential)


def plot_history_duration_relative(url, sequential):
    y_label = "relative e2e testing time [%]"
    file = "../duration_relative" + output_format

    labels = get_labels_git(url)

    df_dynamic = pd.read_sql(
        'SELECT c.repo_id as repository, dynamic_duration * 100.0 / retest_all_duration as y, \'dynamic\' as algorithm '
        'FROM testreport_extended join "Commit" c on c.id = commit ORDER BY commit', url)

    df_static = pd.read_sql(
        'SELECT c.repo_id as repository, static_duration * 100.0 / retest_all_duration as y, \'static\' as algorithm '
        'FROM testreport_extended join "Commit" c on c.id = commit ORDER BY commit', url)

    df = pd.concat([df_dynamic, df_static])

    boxplot(df, labels, y_label, file, ["#E37222", "#A2AD00"], sequential=sequential)


def plot_history_efficiency_repo(url, sequential):
    reg_label = "linear regression x~log(y)"
    y_label = "average relative e2e testing time [%]"
    x_label = "average absolute e2e testing time of retest-all [s]"
    file = "../efficiency_repo"

    df_dynamic = pd.read_sql(
        'SELECT retest_all_mean as x, dynamic_mean_relative * 100.0 as y, \'dynamic\' as algorithm FROM duration',
        url)

    df_static = pd.read_sql(
        'SELECT retest_all_mean as x, static_mean_relative * 100.0 as y, \'static\' as algorithm FROM duration',
        url)

    scatterplot([df_dynamic, df_static], ["dynamic - " + reg_label, "static - " + reg_label], x_label, y_label,
                file + output_format,
                ["#E37222", "#A2AD00"], sequential=("single" in url_git))

    scatterplot([df_dynamic, df_static], ["dynamic - " + reg_label, "static - " + reg_label], x_label, y_label,
                file + "_with_regression" + output_format,
                ["#E37222", "#A2AD00"], regression=True, sequential=sequential)


def target_count_absolute(url, sequential):
    y_label = "absolute number of tests"
    file = "../selected_targets_absolute"

    labels = get_labels_git(url)

    df_retest_all_unit = pd.read_sql(
        'SELECT c.repo_id as repository, retest_all_count as y, \'retest-all - unit\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
        url)

    df_dynamic_unit = pd.read_sql(
        'SELECT c.repo_id as repository, dynamic_count as y, \'dynamic - unit\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
        url)

    df_static_unit = pd.read_sql(
        'SELECT c.repo_id as repository, static_count as y, \'static - unit\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
        url)

    df_retest_all_integration = pd.read_sql(
        'SELECT c.repo_id as repository, retest_all_count as y, \'retest-all - integration\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
        url)

    df_dynamic_integration = pd.read_sql(
        'SELECT c.repo_id as repository, dynamic_count as y, \'dynamic - integration\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
        url)

    df_static_integration = pd.read_sql(
        'SELECT c.repo_id as repository, static_count as y, \'static - integration\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
        url)

    df_dynamic = pd.concat([df_retest_all_unit, df_retest_all_integration, df_dynamic_unit, df_dynamic_integration])
    df_static = pd.concat([df_retest_all_unit, df_retest_all_integration, df_static_unit, df_static_integration])

    boxplot(df_dynamic, labels, y_label, file + "_dynamic" + output_format,
            ["#E0DED4", "#ADABA1", "#E98C4A", "#B65C1B"],
            sequential=sequential)
    boxplot(df_static, labels, y_label, file + "_static" + output_format, ["#E0DED4", "#ADABA1", "#B4BE26", "#818B00"],
            sequential=sequential)
    # boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])


def target_count_relative(url, sequential):
    y_label = "relative number of tests [%]"
    file = "../selected_targets_relative"

    labels = get_labels_git(url)

    df_dynamic_unit = pd.read_sql(
        'SELECT c.repo_id as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic - unit\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
        url)

    df_static_unit = pd.read_sql(
        'SELECT c.repo_id as repository, static_count * 100.0 / retest_all_count as y, \'static - unit\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'UNIT\' ORDER BY commit',
        url)

    df_dynamic_integration = pd.read_sql(
        'SELECT c.repo_id as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic - integration\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
        url)

    df_static_integration = pd.read_sql(
        'SELECT c.repo_id as repository, static_count * 100.0 / retest_all_count as y, \'static - integration\' as algorithm '
        'FROM target_count join "Commit" c on c.id = commit WHERE target = \'INTEGRATION\' ORDER BY commit',
        url)

    df = pd.concat([df_dynamic_unit, df_dynamic_integration, df_static_unit, df_static_integration])

    boxplot_history_with_observations(df, labels, y_label, file + output_format,
                                      ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
                                      sequential=sequential, figsize=(24, 15),
                                      legend_anchor=(1.0, 0.8, 0.1, 0.1))
    boxplot(df, labels, y_label, file + "_boxplot" + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            sequential=sequential, figsize=(24, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))
    stripplot(df, labels, y_label, file + "_stripplot" + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
              sequential=sequential, figsize=(24, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))


def plot_history_testcases_contains_relation(url, sequential):
    y_label = 'Tests that have been selected'
    file = '../contains_all_tests'

    labels = get_labels_git(url)

    def get_test_diff(retest_all, other):
        retest_all_tests = retest_all.splitlines()
        other_tests = other.splitlines()
        return list(set(retest_all_tests) - set(other_tests))

    df_selected_dynamic = pd.read_sql(
        'SELECT c.repo_id as repository, commit, retest_all, dynamic FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
        url)

    df_selected_static = pd.read_sql(
        'SELECT c.repo_id as repository, commit, retest_all, static FROM testcases_selected join "Commit" c ON c.id = commit ORDER BY commit',
        url)

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

    # stripplot(df, labels, y_label, file, ["#E37222"], hue='algorithm')
    stripplot(df_1, labels1, y_label, file + "_1" + output_format,
              ["#E37222"], hue='algorithm', figsize=(18, 15), legend_anchor=(0.3, 0.9, 0.7, 0.1),
              sequential=sequential)
    stripplot(df_2, labels2, "", file + "_2" + output_format, ["#E37222"],
              hue='algorithm', figsize=(3, 15),
              legend=False)


def plot_history_testcases_count_absolute(url, sequential):
    y_label = "absolute number of tests"
    file = "../selected_tests_absolute" + output_format

    labels = get_labels_git()

    df_retest_all = pd.read_sql(
        'SELECT c.repo_id as repository, retest_all_count as y, \'retest-all\' as algorithm '
        'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
        url)

    df_dynamic = pd.read_sql(
        'SELECT c.repo_id as repository, dynamic_count as y, \'dynamic\' as algorithm '
        'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
        url)

    df_static = pd.read_sql(
        'SELECT c.repo_id as repository, static_count as y, \'static\' as algorithm '
        'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
        url)

    df = pd.concat([df_retest_all, df_dynamic, df_static])

    boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"], sequential=sequential)


def plot_history_testcases_count_relative(url, sequential):
    y_label = "relative number of tests [%]"
    file = "../selected_tests_relative"

    labels = get_labels_git(url)

    df_dynamic = pd.read_sql(
        'SELECT c.repo_id as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic\' as algorithm '
        'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
        url)

    df_static = pd.read_sql(
        'SELECT c.repo_id as repository, static_count * 100.0 / retest_all_count as y, \'static\' as algorithm '
        'FROM testcases_count join "Commit" c on c.id = commit ORDER BY commit',
        url)

    df = pd.concat([df_dynamic, df_static])

    boxplot_history_with_observations(df, labels, y_label, file + output_format, ["#E37222", "#A2AD00"],
                                      sequential=sequential, figsize=(22, 15),
                                      legend_anchor=(1.0, 0.8, 0.1, 0.1))
    boxplot(df, labels, y_label, file + "_boxplot" + output_format, ["#E37222", "#A2AD00"],
            sequential=sequential, figsize=(22, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))
    stripplot(df, labels, y_label, file + "_stripplot" + output_format, ["#E37222", "#A2AD00"],
              sequential=sequential, figsize=(22, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))


def plot_history_testcases_different_absolute(url, sequential):
    y_label_selected = 'Tests with different result, selected'
    file_selected = '../different_and_selected_absolute' + output_format

    y_label_not_selected = 'Tests with different result, not selected'
    file_not_selected = '../different_and_not_selected_absolute' + output_format

    labels = get_labels_git(url)

    df_different_retest_all = pd.read_sql(
        'SELECT c.repo_id as repository, retest_all_different , commit as commit FROM testcases_newly_different join "Commit" c '
        'on c.id = commit ORDER BY commit',
        url)

    df_selected_dynamic = pd.read_sql(
        'SELECT  c.repo_id as repository, dynamic, commit as commit FROM testcases_selected join "Commit" c on c.id = '
        'commit ORDER BY commit',
        url)

    df_selected_static = pd.read_sql(
        'SELECT  c.repo_id as repository, static, commit as commit FROM testcases_selected join "Commit" c on c.id = '
        'commit ORDER BY commit',
        url)

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

    stripplot(df_selected, labels, y_label_selected, file_selected, ["#E37222", "#A2AD00"], hue='algorithm',
              sequential=sequential)
    stripplot(df_not_selected, labels, y_label_not_selected, file_not_selected, ["#E37222", "#A2AD00"], hue='algorithm',
              sequential=sequential)


def plot_history_testcases_failed_absolute(url, sequential):
    y_label_selected = 'Newly failed tests, selected'
    file_selected = '../failed_and_selected_absolute' + output_format

    y_label_not_selected = 'Newly failed tests, not selected'
    file_not_selected = '../failed_and_not_selected_absolute' + output_format

    labels = get_labels_git(url)

    def get_test_diff_and_intersection(retest_all, other):
        retest_all_tests = set(retest_all.splitlines())
        other_tests = set(other.splitlines())
        return list(retest_all_tests.difference(other_tests)), list(retest_all_tests.intersection(other_tests))

    df_failed_retest_all = pd.read_sql(
        'SELECT c.repo_id as repository, retest_all_failed , commit as commit FROM testcases_newly_failed join "Commit" c '
        'on c.id = commit ORDER BY commit',
        url)

    df_selected_dynamic = pd.read_sql(
        'SELECT  c.repo_id as repository, dynamic, commit as commit FROM testcases_selected join "Commit" c on c.id = '
        'commit ORDER BY commit',
        url)

    df_selected_static = pd.read_sql(
        'SELECT  c.repo_id as repository, static, commit as commit FROM testcases_selected join "Commit" c on c.id = '
        'commit ORDER BY commit',
        url)

    selected_dynamic = []
    not_selected_dynamic = []
    selected_static = []
    not_selected_static = []

    failed_retest_all_count = {}

    raw_failed_retest_all = df_failed_retest_all.to_dict(orient='records')
    raw_selected_dynamic = df_selected_dynamic.to_dict(orient='records')
    raw_selected_static = df_selected_static.to_dict(orient='records')

    assert len(raw_failed_retest_all) == len(raw_selected_dynamic) and len(raw_failed_retest_all) == len(
        raw_selected_static)

    for (retest_all_report, dynamic_report, static_report) in zip(raw_failed_retest_all, raw_selected_dynamic,
                                                                  raw_selected_static):
        repository = retest_all_report['repository']
        commit = retest_all_report['commit']

        if repository not in failed_retest_all_count:
            failed_retest_all_count[repository] = {}
            failed_retest_all_count[repository]["count"] = 0
            failed_retest_all_count[repository]["commits"] = 0
        count = len(set(retest_all_report['retest_all_failed'].splitlines()))
        if count > 0:
            failed_retest_all_count[repository]["count"] += count
            failed_retest_all_count[repository]["commits"] += 1

        (diff_dynamic, intersection_dynamic) = get_test_diff_and_intersection(retest_all_report['retest_all_failed'],
                                                                              dynamic_report['dynamic'])
        (diff_static, intersection_static) = get_test_diff_and_intersection(retest_all_report['retest_all_failed'],
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
        labels[i] += "\n(" + str(failed_retest_all_count[i + 1]["count"]) + " on " + str(
            failed_retest_all_count[i + 1]["commits"]) + ")"

    df_selected = pd.concat([df_selected_dynamic[['repository', 'algorithm', 'y']],
                             df_selected_static[['repository', 'algorithm', 'y']]])

    df_not_selected = pd.concat([df_not_selected_dynamic[['repository', 'algorithm', 'y']],
                                 df_not_selected_static[['repository', 'algorithm', 'y']]])

    stripplot(df_selected, labels, y_label_selected, file_selected, ["#E37222", "#A2AD00"], hue='algorithm',
              sequential=sequential)
    stripplot(df_not_selected, labels, y_label_not_selected, file_not_selected, ["#E37222", "#A2AD00"], hue='algorithm',
              sequential=sequential)


########################################################################################################################
# Mutants plots

def plot_mutants_duration_absolute(url):
    y_label = "absolute e2e testing time [s]"
    file = "../duration_absolute" + output_format

    labels = get_labels_mutants(url)

    df_retest_all = pd.read_sql(
        'SELECT commit as repository, retest_all_duration as y, \'retest-all\' as algorithm FROM mutant_extended',
        url)

    df_dynamic = pd.read_sql(
        'SELECT commit as repository, dynamic_duration as y, \'dynamic\' as algorithm FROM mutant_extended',
        url)

    df_static = pd.read_sql(
        'SELECT commit as repository, static_duration as y, \'static\' as algorithm FROM mutant_extended', url)

    df = pd.concat([df_retest_all, df_dynamic, df_static])

    boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])


def plot_mutants_duration_relative(url):
    y_label = "relative e2e testing time [%]"
    file = "../duration_relative" + output_format

    labels = get_labels_mutants(url)

    df_dynamic = pd.read_sql(
        'SELECT commit as repository, dynamic_duration * 100.0 / retest_all_duration as y, \'dynamic\' as algorithm FROM mutant_extended',
        url)

    df_static = pd.read_sql(
        'SELECT commit as repository, static_duration * 100.0 / retest_all_duration as y, \'static\' as algorithm FROM mutant_extended',
        url)

    df = pd.concat([df_dynamic, df_static])

    boxplot(df, labels, y_label, file, ["#E37222", "#A2AD00"])


def plot_mutants_target_count_absolute(url):
    y_label = "absolute number of tests"
    file = "../selected_targets_absolute"

    labels = get_labels_mutants(url)

    df_retest_all_unit = pd.read_sql(
        'SELECT commit as repository, retest_all_count as y, \'retest-all - unit\' as algorithm '
        'FROM target_count WHERE target = \'UNIT\'',
        url)

    df_dynamic_unit = pd.read_sql(
        'SELECT commit as repository, dynamic_count as y, \'dynamic - unit\' as algorithm '
        'FROM target_count WHERE target = \'UNIT\'',
        url)

    df_static_unit = pd.read_sql(
        'SELECT commit as repository, static_count as y, \'static - unit\' as algorithm '
        'FROM target_count WHERE target = \'UNIT\'',
        url)

    df_retest_all_integration = pd.read_sql(
        'SELECT commit as repository, retest_all_count as y, \'retest-all - integration\' as algorithm '
        'FROM target_count WHERE target = \'INTEGRATION\'',
        url)

    df_dynamic_integration = pd.read_sql(
        'SELECT commit as repository, dynamic_count as y, \'dynamic - integration\' as algorithm '
        'FROM target_count WHERE target = \'INTEGRATION\'',
        url)

    df_static_integration = pd.read_sql(
        'SELECT commit as repository, static_count as y, \'static - integration\' as algorithm '
        'FROM target_count WHERE target = \'INTEGRATION\'',
        url)

    df_dynamic = pd.concat([df_retest_all_unit, df_retest_all_integration, df_dynamic_unit, df_dynamic_integration])
    df_static = pd.concat([df_retest_all_unit, df_retest_all_integration, df_static_unit, df_static_integration])

    boxplot(df_dynamic, labels, y_label, file + "_dynamic" + output_format,
            ["#E0DED4", "#ADABA1", "#E98C4A", "#B65C1B"])
    boxplot(df_static, labels, y_label, file + "_static" + output_format, ["#E0DED4", "#ADABA1", "#B4BE26", "#818B00"])

    # boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])


def plot_mutants_target_count_relative(url):
    y_label = "relative number of tests [%]"
    file = "../selected_targets_relative"

    labels = get_labels_mutants(url)

    df_dynamic_unit = pd.read_sql(
        'SELECT  commit as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic - unit\' as algorithm '
        'FROM target_count WHERE target = \'UNIT\'',
        url)

    df_static_unit = pd.read_sql(
        'SELECT  commit as repository, static_count * 100.0 / retest_all_count as y, \'static - unit\' as algorithm '
        'FROM target_count WHERE target = \'UNIT\'',
        url)

    df_dynamic_integration = pd.read_sql(
        'SELECT  commit as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic - integration\' as algorithm '
        'FROM target_count WHERE target = \'INTEGRATION\'',
        url)

    df_static_integration = pd.read_sql(
        'SELECT  commit as repository, static_count * 100.0 / retest_all_count as y, \'static - integration\' as algorithm '
        'FROM target_count WHERE target = \'INTEGRATION\'',
        url)

    df = pd.concat([df_dynamic_unit, df_dynamic_integration, df_static_unit, df_static_integration])

    boxplot_with_observations(df, labels, y_label, file + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
                              figsize=(22, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))
    boxplot(df, labels, y_label, file + "_boxplot" + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
            figsize=(24, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))
    stripplot(df, labels, y_label, file + "_stripplot" + output_format, ["#E98C4A", "#B65C1B", "#B4BE26", "#818B00"],
              figsize=(24, 15), legend_anchor=(1.0, 0.8, 0.1, 0.1))


def plot_mutants_testcases_contains_relation(url):
    y_label = 'Tests that have been selected'
    file = '../contains_all_tests' + output_format

    labels = get_labels_mutants(url)

    def get_test_diff(retest_all, other):
        retest_all_tests = retest_all.splitlines()
        other_tests = other.splitlines()
        return list(set(retest_all_tests) - set(other_tests))

    df_selected_dynamic = pd.read_sql(
        'SELECT commit as repository, retest_all_mutant_id, dynamic, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
        url)

    df_selected_static = pd.read_sql(
        'SELECT commit as repository, retest_all_mutant_id, static, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
        url)

    not_selected_static = []

    selected_dynamic = df_selected_dynamic.to_dict(orient='records')
    selected_static = df_selected_static.to_dict(orient='records')

    assert len(selected_static) == len(selected_static)

    for (dynamic_mutant, static_mutant) in zip(selected_dynamic, selected_static):
        assert dynamic_mutant['retest_all_mutant_id'] == static_mutant['retest_all_mutant_id']

        repository = static_mutant['repository']
        descr = static_mutant['mutant']

        diff = get_test_diff(dynamic_mutant['dynamic'], static_mutant['static'])

        not_selected_static.append(
            {'repository': repository, 'mutant': descr, 'algorithm': 'dynamic but not static', 'y': len(diff)})

    df_not_selected_static = pd.DataFrame(not_selected_static)

    df = pd.concat([df_not_selected_static[['repository', 'algorithm', 'y']]])

    stripplot(df, labels, y_label, file, ["#E37222"], hue='algorithm', legend_loc="upper left",
              legend_anchor=(0.5, 0.7, 0.4, 0.3))


def plot_mutants_testcases_count_absolute(url):
    y_label = "absolute number of tests"
    file = "../selected_tests_absolute" + output_format

    labels = get_labels_mutants()

    df_retest_all = pd.read_sql(
        'SELECT commit as repository, retest_all_count as y, \'retest-all\' as algorithm FROM testcases_count',
        url)

    df_dynamic = pd.read_sql(
        'SELECT commit as repository, dynamic_count as y, \'dynamic\' as algorithm FROM testcases_count', url)

    df_static = pd.read_sql(
        'SELECT commit as repository, static_count as y, \'static\' as algorithm FROM testcases_count', url)

    df = pd.concat([df_retest_all, df_dynamic, df_static])

    boxplot(df, labels, y_label, file, ["#DAD7CB", "#E37222", "#A2AD00"])


def plot_mutants_count_relative(url):
    y_label = "relative number of tests [%]"
    file = "../selected_tests_relative"

    labels = get_labels_mutants(url)

    df_dynamic = pd.read_sql(
        'SELECT commit as repository, dynamic_count * 100.0 / retest_all_count as y, \'dynamic\' as algorithm FROM testcases_count',
        url)

    df_static = pd.read_sql(
        'SELECT commit as repository, static_count * 100.0 / retest_all_count as y, \'static\' as algorithm FROM testcases_count',
        url)

    df = pd.concat([df_dynamic, df_static])

    boxplot_with_observations(df, labels, y_label, file + output_format, ["#E37222", "#A2AD00"], figsize=(22, 15),
                              legend_anchor=(1.0, 0.8, 0.1, 0.1))
    boxplot(df, labels, y_label, file + "_boxplot" + output_format, ["#E37222", "#A2AD00"], figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1))
    stripplot(df, labels, y_label, file + "_stripplot" + output_format, ["#E37222", "#A2AD00"], figsize=(22, 15),
              legend_anchor=(1.0, 0.8, 0.1, 0.1))

def plot_mutants_testcases_failed_absolute(url):
    y_label_selected = 'Failed tests, selected'
    file_selected = '../failed_and_selected_absolute'

    y_label_not_selected = 'Failed tests, not selected'
    file_not_selected = '../failed_and_not_selected_absolute'

    labels = get_labels_mutants(url)

    df_failed_retest_all = pd.read_sql(
        'SELECT commit as repository, retest_all_mutant_id, retest_all_failed , descr as mutant FROM testcases_failed WHERE descr != \'baseline\' ORDER BY commit, descr',
        url)

    df_selected_dynamic = pd.read_sql(
        'SELECT commit as repository, retest_all_mutant_id, dynamic, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
        url)

    df_selected_static = pd.read_sql(
        'SELECT commit as repository, retest_all_mutant_id, static, descr as mutant FROM testcases_selected WHERE descr != \'baseline\' ORDER BY commit, descr',
        url)

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
              ["#E37222", "#A2AD00"], hue='algorithm', figsize=(18, 15), legend_anchor=(0.1, 0.9, 0.2, 0.1))
    stripplot(df_not_selected_2, labels2, "", file_not_selected + "_2" + output_format, ["#E37222", "#A2AD00"],
              hue='algorithm', figsize=(3, 15),
              legend=False)
    # stripplot(df_not_selected, labels, y_label_not_selected, file_not_selected + output_format,
    #          ["#E37222", "#A2AD00"], hue='algorithm', figsize=(17, 15))


def plot_mutants_percentage_failed(url):
    y_label = "failed tests of selected tests [%]"
    file = "../selected_tests_percentage_failed"

    labels = get_labels_mutants(url, count=False)

    df_retest_all = pd.read_sql(
        'SELECT commit as repository, retest_all_count_failed * 100.0 / retest_all_count as y, \'retest-all\' as algorithm FROM testcases_count'
        ' WHERE retest_all_count != 0 and dynamic_count != 0 and static_count != 0',
        url)

    df_dynamic = pd.read_sql(
        'SELECT commit as repository, dynamic_count_failed * 100.0 / dynamic_count as y, \'dynamic\' as algorithm FROM testcases_count'
        ' WHERE retest_all_count != 0 and dynamic_count != 0 and static_count != 0',
        url)

    df_static = pd.read_sql(
        'SELECT commit as repository, static_count_failed * 100.0 / static_count as y, \'static\' as algorithm FROM testcases_count'
        ' WHERE retest_all_count != 0 and dynamic_count != 0 and static_count != 0',
        url)

    df = pd.concat([df_retest_all, df_dynamic, df_static])

    boxplot(df, labels, y_label, file + output_format, ["#DAD7CB", "#E37222", "#A2AD00"], figsize=(22, 15),
            legend_anchor=(1.0, 0.8, 0.1, 0.1))

########################################################################################################################
# Plotting utilities

def boxplot(df, labels, y_label, file, palette=None, hue='algorithm', figsize=(20, 15), legend=True,
            legend_loc='best', legend_anchor=None, sequential=False):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.boxplot(data=df,
                     x='repository',
                     y='y',
                     hue=hue,
                     showmeans=True,
                     width=0.75,
                     meanprops={
                         "marker": "v",
                         "markerfacecolor": "white",
                         "markeredgecolor": "black",
                         "markersize": "16"
                     },
                     fliersize=14,
                     palette=palette)
    ax.set_xticklabels(labels=labels, rotation='vertical')
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if sequential:
        plt.figtext(0.01, 0.02, 'single-threaded', color='grey', rotation="vertical")
    if legend:
        plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def boxplot_with_observations(df, labels, y_label, file, palette=None, hue='algorithm', figsize=(20, 15),
                              legend=True,
                              legend_loc='best', legend_anchor=None, sequential=False):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.boxplot(data=df,
                     x='repository',
                     y='y',
                     hue=hue,
                     showmeans=True,
                     width=0.75,
                     meanprops={
                         "marker": "v",
                         "markerfacecolor": "white",
                         "markeredgecolor": "black",
                         "markersize": "16"
                     },
                     fliersize=14,
                     palette=palette)

    sns.stripplot(ax=ax,
                  data=df,
                  x='repository',
                  y='y',
                  hue=hue,
                  dodge=True,
                  jitter=.3,
                  size=8,
                  linewidth=1,
                  palette=palette,
                  legend=False)

    ax.set_xticklabels(labels=labels, rotation='vertical')
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if sequential:
        plt.figtext(0.01, 0.02, 'single-threaded', color='grey', rotation="vertical")
    if legend:
        plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def barplot(df, labels, y_label, file, palette, hue='algorithm', figsize=(20, 15), single_threaded=False):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.barplot(data=df,
                     x='repository',
                     y='y',
                     hue=hue,
                     # showmeans=True,
                     # width=0.75,
                     # meanprops={
                     #    "marker": "v",
                     #    "markerfacecolor": "white",
                     #    "markeredgecolor": "black",
                     #    "markersize": "8"
                     # },
                     palette=palette)
    ax.set_xticklabels(labels=labels)
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if single_threaded:
        plt.figtext(0.01, 0.02, 'single-threaded', color='grey', rotation="vertical")
    plt.legend(loc='best')
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def stripplot(df, labels, y_label, file, palette=None, hue='algorithm', figsize=(20, 15), legend=True,
              legend_loc='best', legend_anchor=None, sequential=False):
    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.stripplot(data=df,
                       x='repository',
                       y='y',
                       hue=hue,
                       dodge=True,
                       jitter=.3,
                       size=8,
                       linewidth=1,
                       palette=palette)
    ax.set_xticklabels(labels=labels, rotation='vertical')
    ax.set_xlabel("")
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if legend:
        plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    if sequential:
        plt.figtext(0.01, 0.02, 'single-threaded', color='grey', rotation="vertical")
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


def scatterplot(df_raw, labels, x_label, y_label, file, palette=None, hue='algorithm', figsize=(20, 15),
                x_scale='linear',
                y_scale='linear', legend=True,
                legend_loc='best', legend_anchor=None, regression=False, sequential=False):
    df = pd.concat(df_raw)

    sns.set_style("whitegrid")
    sns.set_context("talk", font_scale=2.0)
    plt.figure(figsize=figsize)
    ax = sns.scatterplot(data=df,
                         x='x',
                         y='y',
                         hue=hue,
                         linewidth=1,
                         edgecolor="black",
                         palette=palette,
                         legend='full')
    if regression:
        for i in range(len(df_raw)):
            ax = sns.regplot(
                data=df_raw[i], x="x", y="y", logx=True, label=labels[i],
                scatter=False, truncate=False, order=1, color=palette[i],
            )

    ax.set_xscale(x_scale)
    ax.set_yscale(y_scale)

    ax.set_xlabel(x_label)
    ax.set_ylabel(y_label)
    ax.get_yaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.get_xaxis().set_minor_locator(mpl.ticker.AutoMinorLocator())
    ax.grid(which='major', linewidth=1.0)
    ax.grid(which='minor', linewidth=0.5)
    if legend:
        plt.legend(loc=legend_loc, bbox_to_anchor=legend_anchor)
    else:
        plt.legend([], [], frameon=False)
    if sequential:
        plt.figtext(0.01, 0.02, 'single-threaded', color='grey')
    plt.tight_layout(pad=0.2)
    plt.savefig(file)


########################################################################################################################
# Commands: TODO