#include "ex.h"
#include "ui_ex.h"
#include <QFileDialog>

ex::ex(QWidget *parent) :
    QMainWindow(parent),
    ui(new Ui::ex)
{
    ui->setupUi(this);
    ui->comboBox->addItems(QStringList() << "4800" << "9600" <<"14400" << "19200" << "38400" << "57600"<<"115200");
    ui->spinBox_8->setValue(w.mb_rtu);

    QTextCodec *codec = QTextCodec::codecForName("UTF-8");
    QTextCodec::setCodecForTr(codec);
    QTextCodec::setCodecForCStrings(codec);
    ui->comboBox_3->addItems(QStringList() << "COM1" << "COM2" << "COM3" <<
    "COM4" << "COM5"<<"COM6"<<"COM7"<<"COM8"<<"COM9"<<"\\.\COM10"<<"\\.\COM11"<<"\\.\COM12");
    w.Baud1=BAUD19200;
    w.port_name="COM6";
    w.mb_rtu =301;
    ui->comboBox->setCurrentIndex(3);
    ui->comboBox_3->setCurrentIndex(5);
     w.ini_c();
    ui->spinBox_8->setValue(w.mb_rtu);
    w.enn = 0;
    QStringList  gg;
    gg << "нет арх"<< "1 мин"<< "2 мин"<< "5 мин" << "10 мин" << "20 мин" <<
         "30 мин" << "1 час"<<"2 часа"<<"4 часа"<<" 6 часов"<<"8 часов"<<"12 часов"<<"24 часа"
          <<"10 секунд"<<"20 секунд"<<"30 секунд";
    ui->set_t1_box->addItems(gg);
    ui->set_t2_box->addItems(gg);
    //dds2 = new QTimer(this);
    //connect(dds2, SIGNAL(timeout()),this, SLOT(rec2()));
    //dds2->start(700);
    for(int k=0;k<12;k++){
     ui->table_kp->setItem(k,0,new QTableWidgetItem(QString::number(k)));
     ui->table_kp->setItem(k,1,new QTableWidgetItem(QString::number(k)));
    }
    for (int i = 0; i < w2.kns.size(); ++i) {
          ss = w2.kns[i];
          ui->comboBox_6->addItem(ss[0]);
         }
    w.ss=w2.kns[0];
    moduleProgramPhase = ModuleProgramIdle;
    moduleProgramFrameIndex = 0;
    moduleProgramPollCount = 0;
    moduleProgramCancelled = false;
    moduleProgramTimeoutTimer = new QTimer(this);
    moduleProgramTimeoutTimer->setSingleShot(true);
    connect(moduleProgramTimeoutTimer, SIGNAL(timeout()),
            this, SLOT(moduleProgramTimeout()));
    ui->moduleSlot->setValue(0);
    ui->moduleProgress->setValue(0);
    ui->moduleCancel->setEnabled(false);
}

ex::~ex()
{
    delete ui;
}

void ex::on_comboBox_6_currentIndexChanged(int index)
{
    sht(index);
    w.ss=w2.kns[index];
}
void ex::sh(bool par)
{
    QString ss,s1;
    unsigned int k;
    if (par)    ss="->";    else    ss="<-"  ;
    if(w.com_S)if (par){
        for(k=0;k<22;k++) ss +=s1.sprintf("%02x ",w.buf_out[k]);
        ss +='{';
         }else {
        for(k = 0;k < 10;k++)
            ss +=s1.sprintf("%02x ",w.buf_in[k]);
                    ss +='{';
                    };
    if (par){
        if(w.com_S)
            for(k=0;k<w.max_out;k++) ss +=s1.sprintf("0x%02x ",w.buf_out[k+22]);
            else
              for(k=0;k<w.max_out;k++) ss +=s1.sprintf("0x%02x ",w.buf_out[k]);
        }
    else  { for(k=0;k<w.max_in;k++)   ss +=s1.sprintf("0x%02x ",w.buf_in[k]);
    ss +=s1.sprintf(" max_in %d ",w.max_in);}
    if(w.com_S)  ss +='}';
    if(!par)if( w.CRC_ok) ss+=" ok"; else  ss+="ERROR";
    ui->scr->append(ss);
    if (w.enn)
        for(k = 0;k <w.max_in;k++)
             w.buf_in[k]=w.buf_in[k+9];
}
void ex::sht(int k)
{ QString ssr;
    //QStringList ss;
     u16 j;
    ss = w2.kns[k];
   ui->lineEdit_115->setText(ss[1]);
   ui->lineEdit_116->setText(ss[6]);
   ui->lineEdit_117->setText(ss[7]);
   ui->lineEdit_118->setText(ss[8]);
   j=ss[2].toInt();
   switch(j){
   case 3:ssr="1200";break;
   case 4:ssr="2400";break;
   case 5:ssr="4800";break;
   case 6:ssr="9600";break;
   case 7:ssr="14400";break;
   case 8:ssr="19200";break;
   case 9:ssr="38400";break;
   case 10:ssr="56000";break;
   case 11:ssr="57600";break;
   }
   ui->lineEdit_119->setText(ssr);
   j=ss[4].toInt();
   switch(j){
   case 0:ssr="1";break;
   case 1:ssr="1,5";break;
   case 2:ssr="2";break;
      }
   ui->lineEdit_120->setText(ssr);
   j=ss[5].toInt();
   switch(j){
   case 0:ssr="no";break;
   case 1:ssr="even";break;
   case 2:ssr="odd";break;
      }
    ui->lineEdit_121->setText(ssr);
    j=ss[9].toInt();
    switch(j){
    case 1:ssr="rs485_1";break;
    case 2:ssr="rs485_2";break;
    case 3:ssr="rs232_1";break;
    case 4:ssr="rs232_2";break;
       }
     ui->lineEdit_122->setText(ssr);
     ui->lineEdit_123->setText(ss[10]);
}

void ex::on_pushButton_166_clicked()
{

    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_csq()));
  w.rd_reg(31934,2);
  sh(1);
}
void ex::sh_csq()
{
    sh(0);
    float dat;
    dat = w.get_real(7+w.ElamFlag); //
    ui->lineEdit_137->setText(QString::number(dat));
}

void ex::on_pushButton_167_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_s_sim()));
    w.rd_reg(31922,3);
    sh(1);
}
void ex::sh_s_sim()
{
    sh(0);
    WORD hh;
    hh = w.get_word1(5+w.ElamFlag); //
    ui->lineEdit_149->setText(QString::number(hh));
    hh = w.get_word1(7+w.ElamFlag); //
    ui->lineEdit_138->setText(QString::number(hh));
   }

void ex::on_pushButton_171_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ip()));
    w.rd_reg(44889,7);//
     sh(1);
}
void ex::sh_ip()
{
    sh(0);
    WORD hh;
    QString ff;
    hh = w.get_word1(5+w.ElamFlag);    ff += QString::number(hh)+ ".";
    hh = w.get_word1(7+w.ElamFlag);    ff += QString::number(hh) + ".";
    hh = w.get_word1(9+w.ElamFlag);    ff += QString::number(hh) + ".";
    hh = w.get_word1(11+w.ElamFlag);   ff += QString::number(hh);
    ui->lineEdit_151->setText(ff);
    hh = w.get_word1(13+w.ElamFlag);
    ui->lineEdit_152->setText(QString::number(hh,10));
    hh = w.get_word1(15+w.ElamFlag);
    ui->lineEdit_150->setText(QString::number(hh,10));
    hh = w.get_word1(17+w.ElamFlag);
    ui->lineEdit_153->setText(QString::number(hh,10));
}

void ex::on_pushButton_174_clicked()
{
    QString ff;
   ff += QString::number(217)+ "." +QString::number(198)+ "." +QString::number(10)+ "." +QString::number(164);
   ui->lineEdit_151->setText(ff);
   ui->lineEdit_152->setText(QString::number(5000,10));
}

void ex::on_pushButton_172_clicked()
{
    unsigned int tt[16];
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    QStringList ws=ui->lineEdit_151->text().split(".");
    for(int k=0;k<ws.count();k++)
        tt[k]=ws.at(k).toInt();
    tt[4]=ui->lineEdit_152->text().toInt();
    tt[5]=ui->lineEdit_150->text().toInt();
    tt[6]=ui->lineEdit_153->text().toInt();
    w.wr_mas(44889,7,&tt[0]);
      sh(1);
}

void ex::on_comboBox_currentIndexChanged(int index)
{
    switch (index){
    case 0: w.Baud1 = BAUD4800; break;
    case 1: w.Baud1 = BAUD9600; break;
    case 2: w.Baud1 = BAUD14400; break;
    case 3: w.Baud1 = BAUD19200; break;
    case 4: w.Baud1 = BAUD38400; break;
    case 5: w.Baud1 = BAUD56000; break;
    case 6: w.Baud1 = BAUD57600; break;
    case 7: w.Baud1 = BAUD115200; break;
               }
w.ini_c();
}

void ex::on_comboBox_3_currentIndexChanged(const QString &arg1)
{
    w.port_name=arg1;
}

void ex::on_pushButton_clicked()
{
    if( w.ini_c()==true)
     ui->label_4->setText("ok");
        else  ui->label_4->setText("ERROR");
}

void ex::on_pushButton_27_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ts()));
    w.rd_reg(30001,1);//
    sh(1);
}
void ex::sh_ts()
{
   BYTE k;
   QString s1;
   sh(0);
   WORD dat;
   qDebug("max_inS: %d", 5);
   dat = w.get_word1(5+w.ElamFlag); //
   //ui->label_28->setText(QString::number(dat,16));
   ui->textEdit->clear();
   for(k=0;k<16;k++){
       //s1 = "N:"+ QString::number(k)+" ";
       s1 = QString("%1:").arg(k+1,2);
   if((dat & (1<<k)))ui->textEdit->append(s1+"1");
       else ui->textEdit->append(s1+".");
                  }

}

void ex::on_pushButton_41_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tii3()));
    w.rd_reg(30015,16);//
    sh(1);
}
void ex::sh_tii3()
{

    sh(0);
    WORD hh,k;
    ui->textEdit_4->clear();
    for(k=0;k<16;k++){
     hh = w.get_word1(5+w.ElamFlag+k*2); //
     ui->textEdit_4->append(QString("%1:%2").arg(k+1,2).arg(hh,6));
    }
}

void ex::on_checkBox_28_clicked()
{
    if(ui->checkBox_28->isChecked())w.com_S=1;else w.com_S=0;
}

void ex::on_pushButton_40_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tit12()));
    w.rd_reg(30002,12);//
    sh(1);
}
void ex::sh_tit12()
{

    sh(0);
    WORD hh,k;
    ui->textEdit_3->clear();
    for(k=0;k<12;k++){
     hh = w.get_word1(5+w.ElamFlag+k*2); //
     ui->textEdit_3->append(QString("%1:%2").arg(k+1,2).arg(hh,-6));
    }
}

void ex::on_pushButton_43_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tit_float()));
    w.rd_reg(30033,24);//
    sh(1);
}
void ex::sh_tit_float()
{

    sh(0);
    WORD k;
    float dat;
    ui->textEdit_5->clear();
    for(k=0;k<12;k++){
        dat = w.get_real(7+k*4+w.ElamFlag);
        //if(dat==0)test_0[2]++;
        ui->textEdit_5->append(QString("%1:%2").arg(k+1,2).arg(dat,4,'F',3));
    }
//ui->lineEdit_23->setText(QString("%1").arg(test_0[2]));
}

void ex::on_pushButton_2_clicked()
{

}

void ex::on_lineEdit_118_textChanged(const QString &arg1)
{

}

void ex::on_lineEdit_118_editingFinished()
{
     w.ss[8]=ui->lineEdit_118->text();
}

//void ex::on_pushButton_4_clicked()
//{

//}

//void ex::on_lineEdit_14_cursorPositionChanged(int arg1, int arg2)
//{

//}

//void ex::on_pushButton_13_clicked()
//{

//}

void ex::on_get_ver_rd_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ver()));
    w.rd_reg(31963,1);//
    sh(1);
}
void ex::sh_ver()
{

    sh(0);
    unsigned int dat;
    dat = w.get_word1(5+w.ElamFlag);
    ui->get_ver_line->setText(QString::number(dat));

}
//void ex::set_func( void (*f)(void))
//{
//    //disconnect(&w,SIGNAL(s_rd()),0,0);
//    //connect(&w,SIGNAL(s_rd()),this,SLOT((*f)()));
//}


void ex::on_ini_par_ty_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_ty(847,1);
    sh(1);
}

void ex::on_time_rd_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_d()));
    w.rd_reg(41425,6);//
    sh(1);
   }
void ex::sh_d()
{
    sh(0);
    WORD ss;
     ss = w.get_word1(5+w.ElamFlag); ui->time_year->setText(QString::number(ss));
     ss = w.get_word1(7+w.ElamFlag); ui->time_m->setText(QString::number(ss));
     ss = w.get_word1(9+w.ElamFlag); ui->time_day->setText(QString::number(ss));
     ss = w.get_word1(11+w.ElamFlag); ui->time_hour->setText(QString::number(ss));
     ss = w.get_word1(13+w.ElamFlag); ui->time_min->setText(QString::number(ss));
     ss = w.get_word1(15+w.ElamFlag); ui->time_sek->setText(QString::number(ss));

}

void ex::on_time_wr_clicked()
{
    unsigned  int tt[6];
    QString s;
    WORD rr;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
s = ui->time_year->text(); rr = s.toInt();tt[0]=rr; //max
s = ui->time_m->text(); rr = s.toInt(); tt[1]=rr;  // min
s = ui->time_day->text(); rr = s.toInt();tt[2]=rr; //max
s = ui->time_hour->text(); rr = s.toInt(); tt[3]=rr;  // min
s = ui->time_min->text(); rr = s.toInt(); tt[4]=rr;  // min
s = ui->time_sek->text(); rr = s.toInt(); tt[5]=rr;  // min
w.wr_mas(41425,6,&tt[0]);//
    sh(1);
}
void ex::sh_ty()
{
    sh(0);

}
void ex::on_time_set_2_clicked()
{
    QString ss;
    QDate dd = QDate::currentDate();
    QTime tt = QTime::currentTime();
    ui->time_year->setText(dd.toString("yyyy"));
    ui->time_m->setText(dd.toString("MM"));
    ui->time_day->setText(dd.toString("dd"));
    ui->time_hour->setText(tt.toString("HH"));
    ui->time_min->setText(tt.toString("mm"));
    ui->time_sek->setText(tt.toString("s"));
}

void ex::on_set_t1_rd_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tii_time()));
    w.rd_reg(40242,5);//
     sh(1);
}
void ex::sh_tii_time() //
   {
    sh(0);
    u16 r2;
   r2=w.get_word1(5+w.ElamFlag);   if(r2<16)   ui->set_t1_box->setCurrentIndex(r2);
    }

void ex::on_set_t1_wr_clicked()
{
    unsigned int tt[7];
    QString s;
    u16 r;
   disconnect(&w,SIGNAL(s_rd()),0,0);
   connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
   r= ui->set_t1_box->currentIndex();  tt[0]=r;
   tt[1]=0;
   w.wr_mas(40242,1,&tt[0]);//
   sh(1);
}

void ex::on_pushButton_51_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_tii_time2()));
    w.rd_reg(40251,1);//
    sh(1);
}
void ex::sh_tii_time2() //
   {
    sh(0);
    u16 r2;
   r2=w.get_word1(5+w.ElamFlag);
   if(r2<14)
   ui->set_t2_box->setCurrentIndex(r2);

   }


void ex::on_tit_rd_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_n_f()));
    w.rd_reg(40017,48);//
    sh(1);
    }
void ex::sh_n_f()
    {
    float dat;
        sh(0);
    for(int k=0;k < 12;k++)
      {
      dat = w.get_real(7 + k*8 + w.ElamFlag);
      ui->table_kp->item(k,0)->setText(QString::number(dat));
      dat = w.get_real(11 + k*8 + w.ElamFlag);
      ui->table_kp->item(k,1)->setText(QString::number(dat));
         }
    }

void ex::on_tit_rd_2_clicked()
{
    float rr;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    unsigned int tt[48];
    for(int k=0;k<12;k++){
        rr= ui->table_kp->item(k,0)->text().toFloat(); w.setMbFloat1(&tt[k*4],rr);
        rr= ui->table_kp->item(k,1)->text().toFloat(); w.setMbFloat1(&tt[k*4+2],rr);
     }
        w.wr_mas(40017,48,&tt[0]);//
    sh(1);
}

void ex::on_tit_setd_clicked()
{
    u16 k;
    for(k=0;k<12;k++){
    ui->table_kp->item(k,0)->setText("6");
    ui->table_kp->item(k,1)->setText("0");
             }
}

void ex::on_pushButton_278_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_mbF()));
    w.rd_reg(40252,2);//
    sh(1);
}
void ex::sh_mbF()
{
    sh(0);
    WORD r2;
     r2 = w.get_word1(5+w.ElamFlag); //
    if(r2 & 0x0001) ui->mb1->setChecked(1);else ui->mb1->setChecked(0);
    if(r2 & 0x0002) ui->mb1_2->setChecked(1);else ui->mb1_2->setChecked(0);
    if(r2 & 0x0004) ui->mb1_3->setChecked(1);else ui->mb1_3->setChecked(0);
    if(r2 & 0x0008) ui->mb1_4->setChecked(1);else ui->mb1_4->setChecked(0);
    if(r2 & 0x0010) ui->mb1_5->setChecked(1);else ui->mb1_5->setChecked(0);
    if(r2 & 0x0020) ui->mb1_6->setChecked(1);else ui->mb1_6->setChecked(0);
    if(r2 & 0x0040) ui->mb1_7->setChecked(1);else ui->mb1_7->setChecked(0);
    if(r2 & 0x0080) ui->mb1_8->setChecked(1);else ui->mb1_8->setChecked(0);
    if(r2 & 0x0100) ui->mb1_9->setChecked(1);else ui->mb1_9->setChecked(0);
    if(r2 & 0x0200) ui->mb1_10->setChecked(1);else ui->mb1_10->setChecked(0);
    if(r2 & 0x0400) ui->mb1_11->setChecked(1);else ui->mb1_11->setChecked(0);
    if(r2 & 0x0800) ui->mb1_12->setChecked(1);else ui->mb1_12->setChecked(0);
    if(r2 & 0x1000) ui->mb1_13->setChecked(1);else ui->mb1_13->setChecked(0);
    if(r2 & 0x2000) ui->mb1_14->setChecked(1);else ui->mb1_14->setChecked(0);
    if(r2 & 0x4000) ui->mb1_15->setChecked(1);else ui->mb1_15->setChecked(0);
    if(r2 & 0x8000) ui->mb1_16->setChecked(1);else ui->mb1_16->setChecked(0);

    r2 = w.get_word1(7+w.ElamFlag); //
   if(r2 & 0x0001) ui->mb2->setChecked(1);else ui->mb2->setChecked(0);
   if(r2 & 0x0002) ui->mb2_2->setChecked(1);else ui->mb2_2->setChecked(0);
   if(r2 & 0x0004) ui->mb2_3->setChecked(1);else ui->mb2_3->setChecked(0);
   if(r2 & 0x0008) ui->mb2_4->setChecked(1);else ui->mb2_4->setChecked(0);
   if(r2 & 0x0010) ui->mb2_5->setChecked(1);else ui->mb2_5->setChecked(0);
   if(r2 & 0x0020) ui->mb2_6->setChecked(1);else ui->mb2_6->setChecked(0);
   if(r2 & 0x0040) ui->mb2_7->setChecked(1);else ui->mb2_7->setChecked(0);
   if(r2 & 0x0080) ui->mb2_8->setChecked(1);else ui->mb2_8->setChecked(0);
   if(r2 & 0x0100) ui->mb2_9->setChecked(1);else ui->mb2_9->setChecked(0);
   if(r2 & 0x0200) ui->mb2_10->setChecked(1);else ui->mb2_10->setChecked(0);
   if(r2 & 0x0400) ui->mb2_11->setChecked(1);else ui->mb2_11->setChecked(0);
   if(r2 & 0x0800) ui->mb2_12->setChecked(1);else ui->mb2_12->setChecked(0);
   if(r2 & 0x1000) ui->mb2_13->setChecked(1);else ui->mb2_13->setChecked(0);
   if(r2 & 0x2000) ui->mb2_14->setChecked(1);else ui->mb2_14->setChecked(0);
   if(r2 & 0x4000) ui->mb2_15->setChecked(1);else ui->mb2_15->setChecked(0);
   if(r2 & 0x8000) ui->mb2_16->setChecked(1);else ui->mb2_16->setChecked(0);


}

void ex::on_pushButton_279_clicked()
{
    unsigned int tt[4];
    QString s;
    u16 r,r1;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
   tt[0]=0;tt[1]=0;
   if(ui->mb1->isChecked()) tt[0] |=0x0001;
   if(ui->mb1_2->isChecked()) tt[0] |=0x0002;
   if(ui->mb1_3->isChecked()) tt[0] |=0x0004;
   if(ui->mb1_4->isChecked()) tt[0] |=0x0008;
   if(ui->mb1_5->isChecked()) tt[0] |=0x0010;
   if(ui->mb1_6->isChecked()) tt[0] |=0x0020;
   if(ui->mb1_7->isChecked()) tt[0] |=0x0040;
   if(ui->mb1_8->isChecked()) tt[0] |=0x0080;
   if(ui->mb1_9->isChecked()) tt[0] |=0x0100;
   if(ui->mb1_10->isChecked()) tt[0] |=0x0200;
   if(ui->mb1_11->isChecked()) tt[0] |=0x0400;
   if(ui->mb1_12->isChecked()) tt[0] |=0x0800;
   if(ui->mb1_13->isChecked()) tt[0] |=0x1000;
   if(ui->mb1_14->isChecked()) tt[0] |=0x2000;
   if(ui->mb1_15->isChecked()) tt[0] |=0x4000;
   if(ui->mb1_16->isChecked()) tt[0] |=0x8000;

   if(ui->mb2->isChecked()) tt[1] |=0x0001;
   if(ui->mb2_2->isChecked()) tt[1] |=0x0002;
   if(ui->mb2_3->isChecked()) tt[1] |=0x0004;
   if(ui->mb2_4->isChecked()) tt[1] |=0x0008;
   if(ui->mb2_5->isChecked()) tt[1] |=0x0010;
   if(ui->mb2_6->isChecked()) tt[1] |=0x0020;
   if(ui->mb2_7->isChecked()) tt[1] |=0x0040;
   if(ui->mb2_8->isChecked()) tt[1] |=0x0080;
   if(ui->mb2_9->isChecked()) tt[1] |=0x0100;
   if(ui->mb2_10->isChecked()) tt[1] |=0x0200;
   if(ui->mb2_11->isChecked()) tt[1] |=0x0400;
   if(ui->mb2_12->isChecked()) tt[1] |=0x0800;
   if(ui->mb2_13->isChecked()) tt[1] |=0x1000;
   if(ui->mb2_14->isChecked()) tt[1] |=0x2000;
   if(ui->mb2_15->isChecked()) tt[1] |=0x4000;
   if(ui->mb2_16->isChecked()) tt[1] |=0x8000;

w.wr_mas(40252,2,&tt[0]);//
sh(1);
}


void ex::on_B_sos_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_sos2()));
    w.rd_reg(31841,32);//
    sh(1);
}
void ex::sh_sos2()
{
    sh(0);
    WORD hh[14],k;
    QString ss;
    ui->textEdit_17->clear();
    for(k=0;k<4;k++){
        hh[0] = w.get_word1(5+k*16+w.ElamFlag); //
        hh[1] = w.get_word1(7+k*16+w.ElamFlag); //
        hh[2] = w.get_word1(9+k*16+w.ElamFlag); //
        hh[3] = w.get_word1(11+k*16+w.ElamFlag); //
        hh[4] = w.get_word1(13+k*16+w.ElamFlag); //
        hh[5] = w.get_word1(15+k*16+w.ElamFlag); //
        hh[6] = w.get_word1(17+k*16+w.ElamFlag); //
        hh[7] = w.get_word1(19+k*16+w.ElamFlag); //
     ss=QString("%1: %2 %3 %4 %5 %5 %6 %7 %8 %9 ").arg(k*8+1,-6).arg(hh[0],-6,16).arg(hh[1],-6,16).arg(hh[2],-6,16)
             .arg(hh[3],-6,16).arg(hh[4],-6,16).arg(hh[5],-6,16).arg(hh[6],-6,16).arg(hh[7],-6,16);
     ui->textEdit_17->append(ss);
                    }
}


void ex::on_B_zam_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_zamb()));
    w.rd_reg(33101,64);//
    sh(1);
}
void ex::sh_zamb()
{
    sh(0);
    float hh[14],k;
    QString ss;
    ui->textEdit_17->clear();
    for(k=0;k<4;k++){
        hh[0] = w.get_real(7+k*32+w.ElamFlag); //
        hh[1] = w.get_real(11+k*32+w.ElamFlag); //
        hh[2] = w.get_real(15+k*32+w.ElamFlag); //
        hh[3] = w.get_real(19+k*32+w.ElamFlag); //
        hh[4] = w.get_real(23+k*32+w.ElamFlag); //
        hh[5] = w.get_real(27+k*32+w.ElamFlag); //
        hh[6] = w.get_real(31+k*32+w.ElamFlag); //
        hh[7] = w.get_real(35+k*32+w.ElamFlag); //
        ss="";
        qDebug("hh %f  %f %f %f", hh[0],hh[1],hh[2],hh[3]);
        for(int l=0;l<8;l++)
        ss+=QString("%1:%2  ").arg(k*8+l+1,-6).arg(hh[l],-6);
        ui->textEdit_17->append(ss);
    //ui->textEdit_17->append(QString("%1: n-%2 ok-%3 er-%4").arg(k*2+2,-6).arg(hh[4],-6).arg(hh[5],-6).arg(hh[7],-6));
                }
}

void ex::on_pushButton_283_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_er2()));
    w.rd_reg(31501,128);//
    sh(1);
}
void ex::sh_er2()
{
    sh(0);
    WORD hh[14],k;
    QString ss;
    ui->textEdit_17->clear();
    for(k=0;k<16;k++){
        hh[0] = w.get_word1(5+k*16+w.ElamFlag); //
        hh[1] = w.get_word1(7+k*16+w.ElamFlag); //
        hh[2] = w.get_word1(9+k*16+w.ElamFlag); //
        hh[3] = w.get_word1(11+k*16+w.ElamFlag); //
        hh[4] = w.get_word1(13+k*16+w.ElamFlag); //
        hh[5] = w.get_word1(15+k*16+w.ElamFlag); //
        hh[6] = w.get_word1(17+k*16+w.ElamFlag); //
        hh[7] = w.get_word1(19+k*16+w.ElamFlag); //
        ss=QString("%1: n-%2 ok-%3 er-%4 ").arg(k*2+1,-6).arg(hh[0],-6).arg(hh[1],-6).arg(hh[3],-6);
        ss+=QString("%1: n-%2 ok-%3 er-%4 ").arg(k*2+2,-6).arg(hh[4],-6).arg(hh[5],-6).arg(hh[7],-6);
     ui->textEdit_17->append(ss);
    //ui->textEdit_17->append(QString("%1: n-%2 ok-%3 er-%4").arg(k*2+2,-6).arg(hh[4],-6).arg(hh[5],-6).arg(hh[7],-6));
                }
}


void ex::on_pushButton_139_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_indzz()));
    w.rd_reg(30401,1);//
    sh(1);
}
void ex::sh_indzz(void) //
{
 sh(0);
 unsigned int dat;
 dat = w.get_word1(5+w.ElamFlag);
 ind_st_zz=dat;
 ui->lineEdit_76->setText(QString::number(ind_st_zz));
 }

void ex::on_pushButton_140_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_arx_zz()));
   w.rd_reg1(0x2000+(ind_st_zz*74),74);
     if(ind_st_zz)ind_st_zz--;else ind_st_zz =546;  //
     ui->lineEdit_76->setText(QString::number(ind_st_zz)); //
    sh(1);
}
void ex::sh_arx_zz(void) //
{
 QString ss;
 float z;
 QDateTime d;
 DWORD datt;
 u16 k,k1;
 sh(0);
 datt = w.get_u32(7+w.ElamFlag);
 d.setTime_t(datt);
 ss = d.toString("dd/MM/yyyy HH:mm:ss ");// + QString::number(datt,16);
 ui->textEdit_17->append(ss);
 ss="";
 z =  w.get_real1(11+w.ElamFlag);
 ss+= QString::number(1)+"{"+ QString::number(z)+"} ";
 z =  w.get_real1(15+w.ElamFlag);
 ss+= QString::number(2)+"{"+ QString::number(z)+"} ";
 z =  w.get_real1(19+w.ElamFlag);
 ss+= QString::number(3)+"{"+ QString::number(z)+"} ";

 z =  w.get_real1(23+w.ElamFlag);
 ss+= QString::number(4)+"{"+ QString::number(z)+"} ";
  ui->textEdit_17->append(ss);


 for(k1=0;k1<4;k1++){
     ss="";
     for(k=0;k<8;k++){
             z =  w.get_real1(11+k*4+k1*32+w.ElamFlag);
             ss+= QString::number(k+1+k1*8)+"{"+ QString::number(z)+"} ";
                      }
    ui->textEdit_17->append(ss);

                      }

}

void ex::on_pushButton_145_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_indzz1()));
    w.rd_reg(30402,1);//
    sh(1);
}
void ex::sh_indzz1(void) //
{
 sh(0);
 unsigned int dat;
 dat = w.get_word1(5+w.ElamFlag);
 ui->lineEdit_77->setText(QString::number(dat));
 ind_st_zz1=dat;
}

void ex::on_pushButton_146_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_arx_zz1()));
   w.rd_reg1(0xD000+(ind_st_zz1*4),4);
     if(ind_st_zz1)ind_st_zz1--;else ind_st_zz1 =1530;  //
     ui->lineEdit_77->setText(QString::number(ind_st_zz1)); //
    sh(1);
}
void ex::sh_arx_zz1(void) //
{
    QString ss;
    sh(0);

    WORD k,z,z1;
    DWORD datt;
    QDateTime d;
    datt = w.get_u32(7+w.ElamFlag);
    d.setTime_t(datt);
    ss = d.toString("dd/MM/yyyy HH:mm:ss ");// + QString::number(datt,16);
     z1 = w.get_word1(9+w.ElamFlag);
     if(z1<33)
        ss+= " rtu:"+ QString::number(z1); //obv
     if(z1==60)
        ss+= " ts:";
     if(z1==70)
        ss+= "reset:";

    z = w.get_word1(11+w.ElamFlag);
   if(z1<33) ss+= " состояние:"+ QString::number(z,16); //obv
   if(z1==60) ss+= " состояние:"+ QString::number(z,16); //obv
   if(z1==70) ss+= " количество:"+ QString::number(z); //obv
     ui->textEdit_17->append(ss);


}


void ex::on_pushButton_144_clicked()
{
    ui->textEdit_17->clear();
}

void ex::on_pushButton_280_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);   connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_ty(847,1);
    sh(1);
}

void ex::on_pushButton_141_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);   connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_ty(10000,1);
    sh(1);
}

void ex::on_pushButton_142_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);   connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_ty(10001,1);
    sh(1);
}

void ex::on_pushButton_149_clicked()
{
    ui->scr->clear();
}

void ex::on_pushButton_147_clicked()
{

}

void ex::on_pushButton_148_clicked()
{

}

void ex::on_p_ind1_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ind_t1()));
    w.rd_reg(30401,1);//
    sh(1);
}
void ex::sh_ind_t1(void) //
{
 sh(0);
 unsigned int dat;
 dat = w.get_word1(5+w.ElamFlag);
 ind_t1=dat;
 ui->l_t1->setText(QString::number(ind_t1));
 }

void ex::on_pushButton_153_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_arx_t1()));
   w.rd_reg1(10000+(ind_t1*18),18);
  // w.rd_reg1(10018,18);
     if(ind_t1)ind_t1--;else ind_t1 =150;  //
     ui->l_t1->setText(QString::number(ind_t1)); //
    sh(1);
}
void ex::sh_arx_t1(void) //
{
    QString ss,ss1,ss2,ss3,ss4,ss5;
    sh(0);
     float ff1,ff2;
    WORD tts[10],k,z;
    DWORD datt;
    QDateTime d;
    datt = w.get_u32(7+w.ElamFlag);
    d.setTime_t(datt);
    ss = d.toString("dd/MM/yyyy HH:mm:ss ");// + QString::number(datt,16);
    for(k=0;k<16;k++){
    z = w.get_word1(9+k*2+w.ElamFlag);
    ss+= QString::number(k+1)+"{"+ QString::number(z)+"} "; //obv
   }
      ui->textEdit_17->append(ss);

}

void ex::on_l_t1_textChanged(const QString &arg1)
{
    ind_t1=arg1.toInt();
}

void ex::on_ini_par_ty_2_clicked()
{
//dd.show();
}

void ex::on_pushButton_52_clicked()
{

}


void ex::on_p_bl_on_clicked()
{
    unsigned int tt[7];
    QString s;
    u16 r;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    tt[0]=ui->sp_slot->value();
    w.wr_mas2(12048,1,&tt[0]);//
    sh(1);
}


void ex::on_p_bl_onn_clicked()
{
    unsigned int tt[7];
    QString s;
    u16 r;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    r= ui->set_t1_box->currentIndex();  tt[0]=r;
    tt[0]=0xa503;
    w.wr_mas2(12050,1,&tt[0]);//
    sh(1);
}


void ex::on_p_bl_off_clicked()
{
    unsigned int tt[7];
    QString s;
    u16 r;
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    r= ui->set_t1_box->currentIndex();  tt[0]=r;
    tt[0]=0xa504;
    w.wr_mas2(12050,1,&tt[0]);//
    sh(1);
}

/*
 * Включает опрос демонстрационного модуля UART7.
 * wr_reg() формирует Modbus function 06; адрес 42501 преобразуется в 2500.
 */
void ex::on_p_poll_on_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_reg(42501,1);
    sh(1);
}

/*
 * Выключает опрос демонстрационного модуля UART7 через TIT[2500]=0.
 */
void ex::on_p_poll_off_clicked()
{
    disconnect(&w,SIGNAL(s_rd()),0,0);
    connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ty()));
    w.wr_reg(42501,0);
    sh(1);
}


void ex::on_p_sos_slot_clicked()
{
        disconnect(&w,SIGNAL(s_rd()),0,0);
        connect(&w,SIGNAL(s_rd()),this,SLOT(sh_ssos()));
        w.rd_reg(32451,4);//
        sh(1);
       }
void ex::sh_ssos(void) //
{
        sh(0);
        u16 r2;
        ui->t_ssos->clear();
        r2=w.get_word1(5+w.ElamFlag);
        ui->t_ssos->append("состояние:" +QString::number(r2));
        r2=w.get_word1(6+w.ElamFlag);
        ui->t_ssos->append("код:" +QString::number(r2));
        r2=w.get_word1(8+w.ElamFlag);
        ui->t_ssos->append("n_slot:" +QString::number(r2));

}


void ex::on_p_sos_slot_2_clicked()
{
        disconnect(&w,SIGNAL(s_rd()),0,0);
        connect(&w,SIGNAL(s_rd()),this,SLOT(sh_s2500()));
        w.rd_reg(32501,5);//
        sh(1);
       }
void ex::sh_s2500(void) //
{
        sh(0);
        u16 r2;
        ui->t_ssos->clear();
        r2=w.get_word1(5+w.ElamFlag);
        ui->t_ssos->append("вкл:" +QString::number(r2));
        r2=w.get_word1(6+w.ElamFlag);
        ui->t_ssos->append("период:" +QString::number(r2));
        r2=w.get_word1(8+w.ElamFlag);
        ui->t_ssos->append("масштаб:" +QString::number(r2));
        r2=w.get_word1(9+w.ElamFlag);
        ui->t_ssos->append("масштаб:" +QString::number(r2));
        r2=w.get_word1(10+w.ElamFlag);
        ui->t_ssos->append("тип float:" +QString::number(r2));
        r2=w.get_word1(11+w.ElamFlag);
        ui->t_ssos->append("arx = 1:" +QString::number(r2));

}

void ex::moduleProgramLog(const QString &message)
{
    ui->moduleLog->append(message);
    statusBar()->showMessage(message);
}

void ex::moduleProgramSend(const QByteArray &frame)
{
    if (frame.isEmpty() || frame.size() > int(sizeof(w.buf_out)))
    {
        moduleProgramFail(tr("Не удалось сформировать кадр"));
        return;
    }
    moduleProgramLastRequest = frame;
    w.max_out = frame.size();
    memcpy(w.buf_out, frame.constData(), frame.size());
    w.CRC_ok = false;
    w.port->write(frame.constData(), frame.size());
    moduleProgramTimeoutTimer->start(3000);
}

void ex::moduleProgramSchedulePoll()
{
    if (++moduleProgramPollCount > 100)
    {
        moduleProgramFail(tr("Тайм-аут ожидания операции"));
        return;
    }
    QTimer::singleShot(100, this, SLOT(moduleProgramPoll()));
}

void ex::moduleProgramPoll()
{
    if (moduleProgramCancelled || moduleProgramPhase == ModuleProgramIdle)
        return;
    moduleProgramSend(ModuleProgramming::makeStatusReadFrame(w.mb_rtu));
}

void ex::moduleProgramFinish(const QString &message, bool success)
{
    moduleProgramPhase = ModuleProgramIdle;
    moduleProgramTimeoutTimer->stop();
    disconnect(&w, SIGNAL(s_rd()), this, SLOT(moduleProgramResponse()));
    ui->moduleOpen->setEnabled(true);
    ui->moduleWrite->setEnabled(!moduleProgramImage.isEmpty());
    ui->moduleCancel->setEnabled(false);
    ui->moduleSlot->setEnabled(true);
    if (success)
        ui->moduleProgress->setValue(ui->moduleProgress->maximum());
    moduleProgramLog(message);
}

void ex::moduleProgramFail(const QString &message)
{
    moduleProgramFinish(tr("Ошибка: %1").arg(message), false);
}

void ex::on_moduleOpen_clicked()
{
    const QString fileName = QFileDialog::getOpenFileName(
        this, tr("Выберите образ модуля"), QString(),
        tr("Образ модуля (*.bin);;Все файлы (*.*)"));
    if (fileName.isEmpty())
        return;

    QString error;
    if (!ModuleProgramming::loadImage(
            fileName, ui->moduleSlot->value(), &moduleProgramImage,
            &moduleProgramInfo, &error))
    {
        moduleProgramImage.clear();
        ui->moduleFile->clear();
        ui->moduleWrite->setEnabled(false);
        moduleProgramFail(error);
        return;
    }

    ui->moduleFile->setText(fileName);
    ui->moduleWrite->setEnabled(true);
    ui->moduleProgress->setValue(0);
    ui->moduleLog->clear();
    moduleProgramLog(tr("Файл: %1 байт, CRC 0x%2, тип %3, версия %4")
        .arg(moduleProgramImage.size())
        .arg(moduleProgramInfo.calculatedCrc, 4, 16, QChar('0'))
        .arg(moduleProgramInfo.type)
        .arg(moduleProgramInfo.version));
}

void ex::on_moduleWrite_clicked()
{
    QString error;
    if (!ModuleProgramming::inspectImage(
            moduleProgramImage, ui->moduleSlot->value(),
            &moduleProgramInfo, &error))
    {
        moduleProgramFail(error);
        return;
    }
    if (!w.port->isOpen())
    {
        moduleProgramFail(tr("COM6 не открыт"));
        return;
    }

    moduleProgramFrames = ModuleProgramming::makeDataWriteFrames(
        moduleProgramImage, w.mb_rtu);
    if (moduleProgramFrames.isEmpty())
    {
        moduleProgramFail(tr("Нет кадров для передачи"));
        return;
    }

    disconnect(&w, SIGNAL(s_rd()), 0, 0);
    connect(&w, SIGNAL(s_rd()), this, SLOT(moduleProgramResponse()));
    moduleProgramPhase = ModuleProgramUpload;
    moduleProgramFrameIndex = 0;
    moduleProgramPollCount = 0;
    moduleProgramCancelled = false;
    ui->moduleProgress->setRange(0, moduleProgramFrames.size() + 5);
    ui->moduleProgress->setValue(0);
    ui->moduleOpen->setEnabled(false);
    ui->moduleWrite->setEnabled(false);
    ui->moduleCancel->setEnabled(true);
    ui->moduleSlot->setEnabled(false);
    moduleProgramLog(tr("Передача в слот %1 через COM6...")
                     .arg(ui->moduleSlot->value()));
    moduleProgramSend(moduleProgramFrames.at(0));
}

void ex::on_moduleCancel_clicked()
{
    moduleProgramCancelled = true;
    moduleProgramFinish(tr("Операция отменена пользователем"), false);
}

void ex::moduleProgramResponse()
{
    if (moduleProgramCancelled || moduleProgramPhase == ModuleProgramIdle)
        return;
    moduleProgramTimeoutTimer->stop();
    if (!w.CRC_ok)
    {
        moduleProgramFail(tr("CRC ответа или ответ не получен"));
        return;
    }

    const QByteArray response(reinterpret_cast<const char *>(w.buf_in),
                              int(w.max_in));
    QString error;

    if (moduleProgramPhase == ModuleProgramVerifyPoll ||
        moduleProgramPhase == ModuleProgramWritePoll ||
        moduleProgramPhase == ModuleProgramStartPoll)
    {
        ModuleProgramming::OperationStatus state;
        if (!ModuleProgramming::parseOperationStatus(response, &state, &error))
        {
            moduleProgramFail(error);
            return;
        }
        moduleProgramLog(tr("Состояние: %1, результат: %2")
            .arg(ModuleProgramming::statusText(state.status))
            .arg(ModuleProgramming::resultText(state.result)));

        if (state.status == ModuleProgramming::StatusError)
        {
            moduleProgramFail(ModuleProgramming::resultText(state.result));
            return;
        }
        if (moduleProgramPhase == ModuleProgramVerifyPoll)
        {
            if (state.status == ModuleProgramming::StatusVerified &&
                state.result == 0 && state.verifyToken != 0)
            {
                if (state.crc != moduleProgramInfo.calculatedCrc)
                {
                    moduleProgramFail(tr("CRC контроллера 0x%1, файла 0x%2")
                        .arg(state.crc, 4, 16, QChar('0'))
                        .arg(moduleProgramInfo.calculatedCrc, 4, 16, QChar('0')));
                    return;
                }
                moduleProgramPhase = ModuleProgramConfirm;
                ui->moduleProgress->setValue(moduleProgramFrames.size() + 2);
                moduleProgramSend(ModuleProgramming::makeConfirmFrame(
                    state.verifyToken, w.mb_rtu));
                return;
            }
        }
        else if (moduleProgramPhase == ModuleProgramWritePoll)
        {
            if (state.status == ModuleProgramming::StatusWritten &&
                state.result == 0)
            {
                ui->moduleProgress->setValue(moduleProgramFrames.size() + 4);
                if (ui->moduleAutoStart->isChecked())
                {
                    moduleProgramPhase = ModuleProgramStartCommand;
                    moduleProgramSend(ModuleProgramming::makeStartFrame(w.mb_rtu));
                }
                else
                    moduleProgramFinish(tr("Модуль успешно записан"), true);
                return;
            }
        }
        else if (state.status == ModuleProgramming::StatusRunning &&
                 state.result == 0)
        {
            moduleProgramFinish(tr("Модуль записан и запущен"), true);
            return;
        }
        moduleProgramSchedulePoll();
        return;
    }

    if (!ModuleProgramming::isWriteAcknowledge(
            moduleProgramLastRequest, response, &error))
    {
        moduleProgramFail(error);
        return;
    }

    if (moduleProgramPhase == ModuleProgramUpload)
    {
        ++moduleProgramFrameIndex;
        ui->moduleProgress->setValue(moduleProgramFrameIndex);
        moduleProgramLog(tr("Передан блок %1 из %2")
            .arg(moduleProgramFrameIndex).arg(moduleProgramFrames.size()));
        if (moduleProgramFrameIndex < moduleProgramFrames.size())
            moduleProgramSend(moduleProgramFrames.at(moduleProgramFrameIndex));
        else
        {
            moduleProgramPhase = ModuleProgramSelect;
            moduleProgramSend(ModuleProgramming::makeSelectFrame(
                ui->moduleSlot->value(), moduleProgramImage.size(), w.mb_rtu));
        }
    }
    else if (moduleProgramPhase == ModuleProgramSelect)
    {
        moduleProgramPhase = ModuleProgramVerifyCommand;
        ui->moduleProgress->setValue(moduleProgramFrames.size() + 1);
        moduleProgramSend(ModuleProgramming::makeVerifyFrame(w.mb_rtu));
    }
    else if (moduleProgramPhase == ModuleProgramVerifyCommand)
    {
        moduleProgramPhase = ModuleProgramVerifyPoll;
        moduleProgramPollCount = 0;
        moduleProgramSchedulePoll();
    }
    else if (moduleProgramPhase == ModuleProgramConfirm)
    {
        moduleProgramPhase = ModuleProgramWriteCommand;
        ui->moduleProgress->setValue(moduleProgramFrames.size() + 3);
        moduleProgramSend(ModuleProgramming::makeWriteFrame(w.mb_rtu));
    }
    else if (moduleProgramPhase == ModuleProgramWriteCommand)
    {
        moduleProgramPhase = ModuleProgramWritePoll;
        moduleProgramPollCount = 0;
        moduleProgramSchedulePoll();
    }
    else if (moduleProgramPhase == ModuleProgramStartCommand)
    {
        moduleProgramPhase = ModuleProgramStartPoll;
        moduleProgramPollCount = 0;
        moduleProgramSchedulePoll();
    }
}

void ex::moduleProgramTimeout()
{
    if (moduleProgramPhase != ModuleProgramIdle)
        moduleProgramFail(tr("Нет ответа от COM6 в течение 3 секунд"));
}


