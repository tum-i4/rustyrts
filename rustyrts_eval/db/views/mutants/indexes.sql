-- this file contains some indexes that may speed up some of the analysis queries

create index if not exists "Mutant_descr_index"
    on "Mutant" (descr);

create index if not exists  "Mutant_report_id_index"
    on "Mutant" (report_id);

create index if not exists  "MutantsReport_name_index"
    on "MutantsReport" (name);

create index if not exists  "MutantsReport_commit_str_index"
    on "MutantsReport" (commit_str);

create index if not exists  "MutantsTestCase_id_suite_id_status_index"
    on "MutantsTestCase" (id, suite_id, status);

create index if not exists  "MutantsTestSuite_id_mutant_id_name_index"
    on "MutantsTestSuite" (id, mutant_id, name);

create index if not exists  "MutantsTestSuite_crashed_index"
    on "MutantsTestSuite" (crashed);

create index if not exists  "TestCase_name_index"
    on "TestCase" (name);

create index if not exists  "TestCase_status_index"
    on "TestCase" (status);

create index if not exists  "TestSuite_name_index"
    on "TestSuite" (name);
