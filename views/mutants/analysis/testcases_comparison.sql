create materialized view testcases_comparison
AS
SELECT not_selected_dynamic.commit,
       not_selected_dynamic.retest_all_mutant_id,
       not_selected_dynamic.descr                                              as descr,

       coalesce(STRING_AGG(not_selected_dynamic.retest_all_name,
                           E'\n'
                           ORDER BY not_selected_dynamic.retest_all_name), '') as failed_but_not_selected_dynamic,

       coalesce(STRING_AGG(not_selected_static.retest_all_name,
                           E'\n'
                           ORDER BY not_selected_static.retest_all_name), '') as failed_but_not_selected_static

FROM (SELECT *
      FROM (mutant_testcase_overview overview left outer join "MutantsTestCase" retest_all_failed
            on overview.retest_all_testcase_id = retest_all_failed.id
                AND retest_all_failed.status =
                    'FAILED')
               left outer join "MutantsTestCase" dynamic_selected
                               on overview.dynamic_testcase_id = dynamic_selected.id
      WHERE dynamic_selected.id is null) as not_selected_dynamic,

     (SELECT *
      FROM (mutant_testcase_overview overview left outer join "MutantsTestCase" retest_all_failed
            on overview.retest_all_testcase_id = retest_all_failed.id
                AND retest_all_failed.status =
                    'FAILED')
               left outer join "MutantsTestCase" static_selected
                               on overview.static_testcase_id = static_selected.id
      WHERE static_selected.id is null) as not_selected_static

WHERE not_selected_static.descr = not_selected_dynamic.descr
  AND not_selected_static.commit = not_selected_dynamic.commit
  AND not_selected_dynamic.retest_all_mutant_id = not_selected_dynamic.retest_all_mutant_id

GROUP BY not_selected_dynamic.commit, not_selected_dynamic.descr, not_selected_dynamic.retest_all_mutant_id,
         not_selected_dynamic.dynamic_mutant_id,
         not_selected_dynamic.static_mutant_id;