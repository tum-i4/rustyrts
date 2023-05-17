create index "Mutant_descr_index"
    on "Mutant" (descr);

create index "Mutant_report_id_index"
    on "Mutant" (report_id);

create index "MutantsReport_name_index"
    on "MutantsReport" (name);

create index "MutantsReport_commit_str_index"
    on "MutantsReport" (commit_str);

create index "MutantsTestCase_id_suite_id_status_index"
    on "MutantsTestCase" (id, suite_id, status);

create index "MutantsTestSuite_id_mutant_id_name_index"
    on "MutantsTestSuite" (id, mutant_id, name);

create index "MutantsTestSuite_crashed_index"
    on "MutantsTestSuite" (crashed);

create index "TestCase_name_index"
    on "TestCase" (name);

create index "TestCase_status_index"
    on "TestCase" (status);

create index "TestSuite_name_index"
    on "TestSuite" (name);
