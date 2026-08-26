#ifndef EX_H
#define EX_H

#include <QMainWindow>
#include "ser.h"
#include "ob.h"
#include "ud.h"
#include "module_programming.h"
//#include "di1.h"
namespace Ui {
class ex;
}

class ex : public QMainWindow
{
    Q_OBJECT

public:
    explicit ex(QWidget *parent = 0);
    ~ex();
    //Di1 dd;
    ser w;
    ob w2;
    QTimer *dds2;
     QStringList ss;
     QStringList ss1;
     u16 ind_st_zz,ind_st_zz1,ind_t1;
     void sh(bool par);
     void sht(int k);
      private slots:
    void on_comboBox_6_currentIndexChanged(int index);

    void on_pushButton_166_clicked();

    void on_pushButton_167_clicked();

    void on_pushButton_171_clicked();

    void on_pushButton_174_clicked();

    void on_pushButton_172_clicked();

    void on_comboBox_currentIndexChanged(int index);

    void on_comboBox_3_currentIndexChanged(const QString &arg1);

    void on_pushButton_clicked();

    void on_pushButton_27_clicked();

    void on_pushButton_41_clicked();

    void on_checkBox_28_clicked();
    void sh_ts();
    void sh_csq();
    void sh_s_sim();
    void sh_ip();
    void sh_tii3();
    void sh_tit12();

    void on_pushButton_40_clicked();

    void on_pushButton_43_clicked();
    void sh_tit_float();

    void on_pushButton_2_clicked();

    void on_lineEdit_118_textChanged(const QString &arg1);

    void on_lineEdit_118_editingFinished();

    //void on_pushButton_4_clicked();

    //void on_lineEdit_14_cursorPositionChanged(int arg1, int arg2);

    //void on_pushButton_13_clicked();

    void on_get_ver_rd_clicked();
    //void set_func(void (*f)(void));
    void sh_ver();

    void on_ini_par_ty_clicked();

    void on_time_rd_clicked();
    void sh_d();

    void on_time_wr_clicked();

    void on_time_set_2_clicked();

    void on_set_t1_rd_clicked();
    void sh_tii_time();

    void on_set_t1_wr_clicked();

    void on_pushButton_51_clicked();

    void on_tit_rd_clicked();
    void sh_n_f();

    void on_tit_rd_2_clicked();
    void sh_tii_time2();

    void on_tit_setd_clicked();

    void on_pushButton_278_clicked();
    void sh_mbF();

    void on_pushButton_279_clicked();

    void on_B_sos_clicked();
    void sh_sos2();

    void on_B_zam_clicked();
    void sh_zamb();

    void on_pushButton_283_clicked();
    void sh_er2();

    void on_pushButton_139_clicked();
    void sh_indzz(void);


    void on_pushButton_140_clicked();
    void sh_arx_zz(void);

    void on_pushButton_145_clicked();
    void sh_indzz1(void);

    void on_pushButton_146_clicked();
    void sh_arx_zz1(void);

    void on_pushButton_144_clicked();

    void on_pushButton_280_clicked();

    void on_pushButton_141_clicked();

    void on_pushButton_142_clicked();

    void on_pushButton_149_clicked();

    void on_pushButton_147_clicked();

    void on_pushButton_148_clicked();

    void on_p_ind1_clicked();
    void sh_ind_t1(void); //

    void on_pushButton_153_clicked();
    void sh_arx_t1(void);

    void on_l_t1_textChanged(const QString &arg1);
    void sh_ty();

    void on_ini_par_ty_2_clicked();

    void on_pushButton_52_clicked();

    void on_p_bl_on_clicked();

    void on_p_bl_onn_clicked();

    void on_p_bl_off_clicked();

    void on_p_poll_on_clicked();

    void on_p_poll_off_clicked();

    void on_p_sos_slot_clicked();
    void sh_ssos(void);
    void on_p_sos_slot_2_clicked();
    void sh_s2500(void);
    void on_moduleOpen_clicked();
    void on_moduleWrite_clicked();
    void on_moduleCancel_clicked();
    void moduleProgramResponse();
    void moduleProgramPoll();
    void moduleProgramTimeout();
private:
    enum ModuleProgramPhase
    {
        ModuleProgramIdle,
        ModuleProgramUpload,
        ModuleProgramSelect,
        ModuleProgramVerifyCommand,
        ModuleProgramVerifyPoll,
        ModuleProgramConfirm,
        ModuleProgramWriteCommand,
        ModuleProgramWritePoll,
        ModuleProgramStartCommand,
        ModuleProgramStartPoll
    };

    void moduleProgramSend(const QByteArray &frame);
    void moduleProgramSchedulePoll();
    void moduleProgramFinish(const QString &message, bool success);
    void moduleProgramFail(const QString &message);
    void moduleProgramLog(const QString &message);

    QByteArray moduleProgramImage;
    QList<QByteArray> moduleProgramFrames;
    QByteArray moduleProgramLastRequest;
    ModuleProgramming::ImageInfo moduleProgramInfo;
    ModuleProgramPhase moduleProgramPhase;
    int moduleProgramFrameIndex;
    int moduleProgramPollCount;
    bool moduleProgramCancelled;
    QTimer *moduleProgramTimeoutTimer;
    Ui::ex *ui;
};

#endif // EX_H
